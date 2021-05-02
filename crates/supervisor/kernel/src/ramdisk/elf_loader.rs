//! The default ELF loader for the kernel.

use elfloader::{ElfLoader, Flags, LoadableHeaders, Rela, TypeRela64, P64};
use relic_abi::{bootstrap::BootstrapInfo, cap::CapabilityErrors};
use relic_utils::align;

use crate::{
    addr::VAddr,
    arch::{capability::paging::L4, globals},
    capability::*,
};

/// Default ELF loader class for the sigma space.
#[derive(CopyGetters, Getters, MutGetters)]
pub struct DefaultElfLoader<'a> {
    vbase: VAddr,
    #[getset(get = "pub", get_mut = "pub")]
    cpool: CapAccessorMut<'a, Cpool>,
    #[getset(get = "pub", get_mut = "pub")]
    untyped: CapAccessorMut<'a, UntypedMemory>,

    /// Get the root page table capability.
    pml4: StoredCap,

    /// Contains the last executable region's virtual address.
    #[getset(get_copy = "pub")]
    exe_section_location: VAddr,
}

impl<'a> DefaultElfLoader<'a> {
    pub fn unwrap(
        self,
    ) -> (
        CapAccessorMut<'a, UntypedMemory>,
        CapAccessorMut<'a, Cpool>,
        StoredCap,
    ) {
        (self.untyped, self.cpool, self.pml4)
    }

    pub fn new(
        vbase: VAddr,
        mut cpool: CapAccessorMut<'a, Cpool>,
        bootstrap_info: &mut BootstrapInfo,
        mut untyped: CapAccessorMut<'a, UntypedMemory>,
    ) -> DefaultElfLoader<'a> {
        let pml4 = StoredCap::pml4_retype_from(&mut untyped, &mut cpool).unwrap();
        bootstrap_info.top_level_pml4 = (pml4.1 as u8).into();

        DefaultElfLoader {
            vbase,
            exe_section_location: 1u64.into(),
            cpool,
            untyped,
            pml4: pml4.0,
        }
    }

    pub fn map_empty_page(
        pml4: &mut CapAccessorMut<'_, L4>,
        untyped: &mut CapAccessorMut<'_, UntypedMemory>,
        cpool: &mut CapAccessorMut<'_, Cpool>,
        page_start_addr: VAddr,
        permissions: MapPermissions,
    ) {
        let page_cap = StoredCap::base_page_retype_from::<[u8; 4096]>(untyped, cpool).unwrap();
        pml4.l4_map(page_start_addr, &page_cap.0, untyped, cpool, permissions)
            .unwrap();
        let mut page_raw = page_cap.0.as_base_page_mut().unwrap();

        let data = page_raw.page_data_mut_raw();
        for i in 0..data.len() {
            data[i] = 0;
        }
    }

    /// Map a capability at the target address.
    pub fn map_cap(
        &mut self,
        cap: &StoredCap,
        vaddr: VAddr,
        perms: MapPermissions,
    ) -> Result<(), CapabilityErrors> {
        self.pml4
            .as_l4_mut()
            .unwrap()
            .l4_map(vaddr, cap, &mut self.untyped, &mut self.cpool, perms)
    }
}

/// Implement this trait for customized ELF loading.
///
/// The flow of ElfBinary is that it first calls `allocate` for all regions
/// that need to be allocated (i.e., the LOAD program headers of the ELF binary),
/// then `load` will be called to fill the allocated regions, and finally
/// `relocate` is called for every entry in the RELA table.
impl<'a> ElfLoader for DefaultElfLoader<'a> {
    /// Allocates a virtual region of `size` bytes at address `base`.
    fn allocate(&mut self, load_headers: LoadableHeaders) -> Result<(), &'static str> {
        for header in load_headers {
            info!(
                target:"elf",
                "allocate base = {:#x}, end = {:#x} size = {:#x} flags = {}",
                header.virtual_addr(),
                header.virtual_addr() + header.mem_size(),
                header.mem_size(),
                header.flags()
            );

            let virt_addr_to_load_at = header.virtual_addr() as usize;
            let virt_addr_to_load_at_page_aligned =
                align::align_down(virt_addr_to_load_at, globals::BASE_PAGE_LENGTH);

            // We load only Ring 3 ELFs. So, add Ring3 permissions as well.
            let mut target_permissions = MapPermissions::empty();
            let perms = header.flags();
            if perms.is_write() {
                target_permissions |= MapPermissions::WRITE;
            }
            if perms.is_execute() {
                target_permissions |= MapPermissions::EXECUTE;
            }

            let end_vaddr_to_load_at_aligned = align::align_up(
                virt_addr_to_load_at + header.mem_size() as usize,
                globals::BASE_PAGE_LENGTH,
            ) as usize;

            // TODO: deal with overlapping regions.
            let total_size = end_vaddr_to_load_at_aligned - virt_addr_to_load_at_page_aligned;
            let virt_addr_to_load_at_page_aligned_vaddr: VAddr =
                virt_addr_to_load_at_page_aligned.into();

            for page_count in 0..(total_size / globals::BASE_PAGE_LENGTH) {
                Self::map_empty_page(
                    &mut self.pml4.as_l4_mut().unwrap(),
                    &mut self.untyped,
                    &mut self.cpool,
                    virt_addr_to_load_at_page_aligned_vaddr
                        + page_count * globals::BASE_PAGE_LENGTH,
                    target_permissions,
                );
            }

            info!(
                target: "elf",
                "allocate done. Start: {:#x}, End: {:#x}",
                virt_addr_to_load_at_page_aligned,
                end_vaddr_to_load_at_aligned,
            )
        }

        Ok(())
    }

    /// Copies `region` into memory starting at `base`.
    /// The caller makes sure that there was an `allocate` call previously
    /// to initialize the region.
    fn load(&mut self, flags: Flags, base: u64, region: &[u8]) -> Result<(), &'static str> {
        let start = self.vbase + base;
        let end = self.vbase + base as usize + region.len();

        if flags.is_execute() {
            self.exe_section_location = start.into();
        }

        let pml4 = &self.pml4.as_l4().unwrap().page_data;
        info!(
                target:"elf", "load region into = {:#x} -- {:#x} (Size: {:#x}), Start PAddr: {:?}",
                start, end, end - start, start.translate(pml4));

        for i in 0..end - start {
            // Because we load everything in a target mapper rather than the current one, we use the mapper provided
            // for getting target locations.
            // TODO: Reduce virt_to_phys calls.
            let result = (start + i).translate(&pml4);
            let target_physical_addr = match result {
                Some(a) => a,
                None => panic!("Unable to translate virtual address {:x}", (start + i)),
            };
            let virt_addr_in_current = target_physical_addr.to_paddr_global();
            let data_to_write = region[i as usize];
            unsafe { *virt_addr_in_current.as_mut_ptr::<u8>() = data_to_write };
        }

        Ok(())
    }

    /// Request for the client to relocate the given `entry`
    /// within the loaded ELF file.
    fn relocate(&mut self, entry: &Rela<P64>) -> Result<(), &'static str> {
        let elf_entry_type = TypeRela64::from(entry.get_type());

        let (target_vaddr, vaddr_in_current) = {
            let pml4 = &self.pml4.as_l4().unwrap().page_data;
            let target_vaddr = self.vbase + entry.get_offset();
            let target_paddr = target_vaddr.translate(pml4).expect("Unable to translate");
            let vaddr_in_current = target_paddr.to_paddr_global();
            (target_vaddr, vaddr_in_current)
        };

        // https://www.intezer.com/blog/elf/executable-and-linkable-format-101-part-3-relocations/
        match elf_entry_type {
            TypeRela64::R_RELATIVE => {
                // This is a relative relocation, add the offset (where we put our
                // binary in the vspace) to the addend and we're done.
                debug!(target:"elf",
                    "R_RELATIVE *{:?} = {:#x}",
                    target_vaddr,
                    self.vbase + entry.get_addend()
                );

                unsafe {
                    *vaddr_in_current.as_mut_ptr::<u64>() = (self.vbase + entry.get_addend()).into()
                };

                Ok(())
            }
            _ => Err("Unexpected relocation encountered"),
        }
    }
}
