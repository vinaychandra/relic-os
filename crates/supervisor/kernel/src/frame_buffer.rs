use relic_abi::{
    bootstrap::{BootstrapInfo, ColorMode},
    cap::CapabilityErrors,
};

use crate::{
    addr::{PAddr, PAddrGlobal, VAddr},
    arch::capability::paging::L4,
    bootboot::bootboot,
    capability::{CapAccessorMut, Cpool, MapPermissions, StoredCap, UntypedMemory},
};

#[derive(Debug)]
pub struct FrameBuffer {
    ptr: PAddrGlobal,
    size: usize,

    width: usize,
    height: usize,
    scanline: usize,

    mode: ColorMode,
}

impl FrameBuffer {
    pub fn new_from_bootboot() -> Self {
        unsafe {
            let ptr: PAddr = PAddr::new(bootboot.fb_ptr);
            Self {
                ptr: ptr.to_paddr_global(),
                size: bootboot.fb_size as _,
                height: bootboot.fb_height as _,
                width: bootboot.fb_width as _,
                scanline: bootboot.fb_scanline as _,
                mode: match bootboot.fb_type {
                    0 => ColorMode::ARGB,
                    1 => ColorMode::RGBA,
                    2 => ColorMode::ABGR,
                    3 => ColorMode::BGRA,
                    _ => panic!("Unknown color mode"),
                },
            }
        }
    }

    pub fn bootstrap_and_map(
        &self,
        untyped: &mut CapAccessorMut<'_, UntypedMemory>,
        cpool_to_store: &mut CapAccessorMut<'_, Cpool>,
        l4: &mut CapAccessorMut<'_, L4>,
        bootstrap_info: &mut BootstrapInfo,
    ) {
        info!(target: "graphics", "Bootstrap VGA frame buffer");
        info!(target: "graphics", "Size of frame buffer: {}", self.size);

        let vga_virt_addr: VAddr = 0x5000_000_0000usize.into();

        let untyped_device = unsafe { UntypedMemory::bootstrap(self.ptr, self.size, true) };
        let cpool_location = cpool_to_store.get_free_index().unwrap();
        let untyped_device = cpool_to_store
            .write_to_if_empty(cpool_location, untyped_device)
            .unwrap();
        let mut untyped_device_write = untyped_device.as_untyped_memory_mut().unwrap();
        info!(target: "graphics", "Created device untyped memory at index {}", cpool_location);

        let num_pages = self.size / crate::arch::globals::BASE_PAGE_LENGTH;
        info!(target: "graphics", "Number of pages to create: {}", num_pages);

        let mut current_cpool = StoredCap::cpool_retype_from(untyped, cpool_to_store).unwrap();
        info!(target: "graphics", "CPool created at index: {}", current_cpool.1);

        for page in 0..num_pages {
            let raw_page = {
                let result = StoredCap::base_page_retype_from::<[u8; 4096]>(
                    &mut untyped_device_write,
                    &mut current_cpool.0.as_cpool_mut().unwrap(),
                    false,
                );
                match result {
                    Ok(a) => a,
                    Err(e) => match e {
                        CapabilityErrors::CapabilitySlotsFull => {
                            current_cpool =
                                StoredCap::cpool_retype_from(untyped, cpool_to_store).unwrap();
                            info!(target: "graphics", "CPool created at index: {}", current_cpool.1);
                            StoredCap::base_page_retype_from::<[u8; 4096]>(
                                &mut untyped_device_write,
                                &mut current_cpool.0.as_cpool_mut().unwrap(),
                                false,
                            )
                            .unwrap()
                        }
                        f => panic!("Unhandled capability error for vga bootstrap: {:?}", f),
                    },
                }
            };

            let mut search_store = false;
            let mut retry_count = 0;
            while retry_count < 2 {
                let result = l4.l4_map(
                    vga_virt_addr + (page * crate::arch::globals::BASE_PAGE_LENGTH),
                    &raw_page.0,
                    untyped,
                    &mut current_cpool.0.as_cpool_mut().unwrap(),
                    if search_store {
                        Some(cpool_to_store)
                    } else {
                        None
                    },
                    MapPermissions::WRITE | MapPermissions::CACHE_DISABLE,
                );

                match result.err() {
                    None => break,
                    Some(e) => match e {
                        CapabilityErrors::CapabilitySlotsFull => {
                            current_cpool =
                                StoredCap::cpool_retype_from(untyped, cpool_to_store).unwrap();
                            info!(target: "graphics", "CPool created at index: {}", current_cpool.1);
                            l4.l4_map(
                                vga_virt_addr + (page * crate::arch::globals::BASE_PAGE_LENGTH),
                                &raw_page.0,
                                untyped,
                                &mut current_cpool.0.as_cpool_mut().unwrap(),
                                if search_store {
                                    Some(cpool_to_store)
                                } else {
                                    None
                                },
                                MapPermissions::WRITE | MapPermissions::CACHE_DISABLE,
                            )
                            .unwrap();
                            break;
                        }
                        CapabilityErrors::CapabilitySearchFailedPartial
                        | CapabilityErrors::CapabilitySearchFailed => {
                            retry_count += 1;
                            if retry_count == 2 {
                                panic!("Unable to find capability");
                            }
                            search_store = true;
                        }
                        f => panic!("Unhandled capability error for vga bootstrap: {:?}", f),
                    },
                }
            }
        }

        bootstrap_info.frame_buffer_paddr = self.ptr.to_paddr().into();
        bootstrap_info.frame_buffer_vaddr = vga_virt_addr.into();
        bootstrap_info.frame_buffer_size = self.size;
        bootstrap_info.frame_buffer_width = self.width;
        bootstrap_info.frame_buffer_height = self.height;
        bootstrap_info.frame_buffer_scanline = self.scanline;
        bootstrap_info.frame_buffer_mode = self.mode;
        info!(target: "graphics", "VGA Bootstrap complete!");
        info!(target: "graphics", "VGA Bootstrap info: {:?}", &self);
    }
}
