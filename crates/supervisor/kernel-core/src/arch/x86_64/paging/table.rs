macro_rules! bit {
    ( $x:expr ) => {
        1 << $x
    };
}

macro_rules! check_flag {
    ($doc:meta, $fun:ident, $flag:ident) => {
        #[$doc]
        pub fn $fun(&self) -> bool {
            self.contains(Self::$flag)
        }
    };
}

use crate::addr::{PAddr, VAddr};

use super::{ADDRESS_MASK, BASE_PAGE_LENGTH};

/// A PML4 table.
/// In practice this has only 4 entries but it still needs to be the size of a 4K page.
pub type PML4 = [PML4Entry; 512];

/// A page directory pointer table.
pub type PDPT = [PDPTEntry; 512];

/// A page directory.
pub type PD = [PDEntry; 512];

/// A page table.
pub type PT = [PTEntry; 512];

/// Given virtual address calculate corresponding entry in PML4.
#[inline]
pub fn pml4_index(addr: VAddr) -> usize {
    ((addr.into(): usize) >> 39) & 0b111111111
}

/// Given virtual address calculate corresponding entry in PDPT.
#[inline]
pub fn pdpt_index(addr: VAddr) -> usize {
    ((addr.into(): usize) >> 30) & 0b111111111
}

/// Given virtual address calculate corresponding entry in PD.
#[inline]
pub fn pd_index(addr: VAddr) -> usize {
    ((addr.into(): usize) >> 21) & 0b111111111
}

/// Given virtual address calculate corresponding entry in PT.
#[inline]
pub fn pt_index(addr: VAddr) -> usize {
    ((addr.into(): usize) >> 12) & 0b111111111
}

bitflags! {
    /// PML4 Entry bits description.
    pub struct PML4Entry: u64 {
        /// Present; must be 1 to reference a page-directory-pointer table
        const PML4_P       = bit!(0);
        /// Read/write; if 0, writes may not be allowed to the 512-GByte region
        /// controlled by this entry (see Section 4.6)
        const PML4_RW      = bit!(1);
        /// User/supervisor; if 0, user-mode accesses are not allowed
        /// to the 512-GByte region controlled by this entry.
        const PML4_US      = bit!(2);
        /// Page-level write-through; indirectly determines the memory type used to
        /// access the page-directory-pointer table referenced by this entry.
        const PML4_PWT     = bit!(3);
        /// Page-level cache disable; indirectly determines the memory type used to
        /// access the page-directory-pointer table referenced by this entry.
        const PML4_PCD     = bit!(4);
        /// Accessed; indicates whether this entry has been used for linear-address translation.
        const PML4_A       = bit!(5);
        /// If IA32_EFER.NXE = 1, execute-disable
        /// If 1, instruction fetches are not allowed from the 512-GByte region.
        const PML4_XD      = bit!(63);
    }
}

impl PML4Entry {
    /// Creates a new PML4Entry.
    ///
    /// # Arguments
    ///
    ///  * `pdpt` - The physical address of the pdpt table.
    ///  * `flags`- Additional flags for the entry.
    pub fn new(pdpt: PAddr, flags: PML4Entry) -> PML4Entry {
        assert!((pdpt.into(): usize) % BASE_PAGE_LENGTH == 0);
        PML4Entry {
            bits: (pdpt.into(): u64) | flags.bits,
        }
    }

    /// Retrieves the physical address in this entry.
    pub fn get_address(self) -> PAddr {
        PAddr::from(self.bits & ADDRESS_MASK)
    }

    check_flag!(doc = "Is page present?", is_present, PML4_P);
    check_flag!(doc = "Read/write; if 0, writes may not be allowed to the 512-GByte region, controlled by this entry (see Section 4.6)",
                is_writeable, PML4_RW);
    check_flag!(doc = "User/supervisor; if 0, user-mode accesses are not allowed to the 512-GByte region controlled by this entry.",
                is_user_mode_allowed, PML4_US);
    check_flag!(doc = "Page-level write-through; indirectly determines the memory type used to access the page-directory-pointer table referenced by this entry.",
                is_page_write_through, PML4_PWT);
    check_flag!(doc = "Page-level cache disable; indirectly determines the memory type used to access the page-directory-pointer table referenced by this entry.",
                is_page_level_cache_disabled, PML4_PCD);
    check_flag!(
        doc =
            "Accessed; indicates whether this entry has been used for linear-address translation.",
        is_accessed,
        PML4_A
    );
    check_flag!(doc = "If IA32_EFER.NXE = 1, execute-disable. If 1, instruction fetches are not allowed from the 512-GByte region.",
                is_instruction_fetching_disabled, PML4_XD);
}

bitflags! {
    /// PDPT Entry bits description.
    pub struct PDPTEntry: u64 {
        /// Present; must be 1 to map a 1-GByte page or reference a page directory.
        const PDPT_P       = bit!(0);
        /// Read/write; if 0, writes may not be allowed to the 1-GByte region controlled by this entry
        const PDPT_RW      = bit!(1);
        /// User/supervisor; user-mode accesses are not allowed to the 1-GByte region controlled by this entry.
        const PDPT_US      = bit!(2);
        /// Page-level write-through.
        const PDPT_PWT     = bit!(3);
        /// Page-level cache disable.
        const PDPT_PCD     = bit!(4);
        /// Accessed; if PDPT_PS set indicates whether software has accessed the 1-GByte page
        /// else indicates whether this entry has been used for linear-address translation
        const PDPT_A       = bit!(5);
        /// Dirty; if PDPT_PS indicates whether software has written to the 1-GByte page referenced by this entry.
        /// else ignored.
        const PDPT_D       = bit!(6);
        /// Page size; if set this entry maps a 1-GByte page; otherwise, this entry references a page directory.
        /// if not PDPT_PS this is ignored.
        const PDPT_PS      = bit!(7);
        /// Global; if PDPT_PS && CR4.PGE = 1, determines whether the translation is global; ignored otherwise
        /// if not PDPT_PS this is ignored.
        const PDPT_G       = bit!(8);
        /// Indirectly determines the memory type used to access the 1-GByte page referenced by this entry.
        const PDPT_PAT     = bit!(12);
        /// If IA32_EFER.NXE = 1, execute-disable
        /// If 1, instruction fetches are not allowed from the 512-GByte region.
        const PDPT_XD      = bit!(63);
    }
}

impl PDPTEntry {
    /// Creates a new PDPTEntry.
    ///
    /// # Arguments
    ///
    ///  * `pd` - The physical address of the page directory.
    ///  * `flags`- Additional flags for the entry.
    pub fn new(pd: PAddr, flags: PDPTEntry) -> PDPTEntry {
        assert!((pd.into(): usize) % BASE_PAGE_LENGTH == 0);
        PDPTEntry {
            bits: (pd.into(): u64) | flags.bits,
        }
    }

    /// Retrieves the physical address in this entry.
    pub fn get_address(self) -> PAddr {
        PAddr::from(self.bits & ADDRESS_MASK)
    }

    check_flag!(doc = "Is page present?", is_present, PDPT_P);
    check_flag!(doc = "Read/write; if 0, writes may not be allowed to the 1-GByte region controlled by this entry.",
                is_writeable, PDPT_RW);
    check_flag!(doc = "User/supervisor; user-mode accesses are not allowed to the 1-GByte region controlled by this entry.",
                is_user_mode_allowed, PDPT_US);
    check_flag!(
        doc = "Page-level write-through.",
        is_page_write_through,
        PDPT_PWT
    );
    check_flag!(
        doc = "Page-level cache disable.",
        is_page_level_cache_disabled,
        PDPT_PCD
    );
    check_flag!(
        doc =
            "Accessed; indicates whether this entry has been used for linear-address translation.",
        is_accessed,
        PDPT_A
    );
    check_flag!(doc = "Indirectly determines the memory type used to access the 1-GByte page referenced by this entry. if not PDPT_PS this is ignored.",
                is_pat, PDPT_PAT);
    check_flag!(doc = "If IA32_EFER.NXE = 1, execute-disable. If 1, instruction fetches are not allowed from the 512-GByte region.",
                is_instruction_fetching_disabled, PDPT_XD);
}

bitflags! {
    /// PD Entry bits description.
    pub struct PDEntry: u64 {
        /// Present; must be 1 to map a 2-MByte page or reference a page table.
        const PD_P       = bit!(0);
        /// Read/write; if 0, writes may not be allowed to the 2-MByte region controlled by this entry
        const PD_RW      = bit!(1);
        /// User/supervisor; user-mode accesses are not allowed to the 2-MByte region controlled by this entry.
        const PD_US      = bit!(2);
        /// Page-level write-through.
        const PD_PWT     = bit!(3);
        /// Page-level cache disable.
        const PD_PCD     = bit!(4);
        /// Accessed; if PD_PS set indicates whether software has accessed the 2-MByte page
        /// else indicates whether this entry has been used for linear-address translation
        const PD_A       = bit!(5);
        /// Dirty; if PD_PS indicates whether software has written to the 2-MByte page referenced by this entry.
        /// else ignored.
        const PD_D       = bit!(6);
        /// Page size; if set this entry maps a 2-MByte page; otherwise, this entry references a page directory.
        const PD_PS      = bit!(7);
        /// Global; if PD_PS && CR4.PGE = 1, determines whether the translation is global; ignored otherwise
        /// if not PD_PS this is ignored.
        const PD_G       = bit!(8);
        /// Indirectly determines the memory type used to access the 2-MByte page referenced by this entry.
        /// if not PD_PS this is ignored.
        const PD_PAT     = bit!(12);
        /// If IA32_EFER.NXE = 1, execute-disable
        /// If 1, instruction fetches are not allowed from the 512-GByte region.
        const PD_XD      = bit!(63);
    }
}

impl PDEntry {
    /// Creates a new PDEntry.
    ///
    /// # Arguments
    ///
    ///  * `pt` - The physical address of the page table.
    ///  * `flags`- Additional flags for the entry.
    pub fn new(pt: PAddr, flags: PDEntry) -> PDEntry {
        assert!(pt.into(): usize % BASE_PAGE_LENGTH == 0);
        PDEntry {
            bits: pt.into(): u64 | flags.bits,
        }
    }

    /// Retrieves the physical address in this entry.
    pub fn get_address(self) -> PAddr {
        PAddr::from(self.bits & ADDRESS_MASK)
    }

    check_flag!(
        doc = "Present; must be 1 to map a 2-MByte page or reference a page table.",
        is_present,
        PD_P
    );
    check_flag!(doc = "Read/write; if 0, writes may not be allowed to the 2-MByte region controlled by this entry",
                is_writeable, PD_RW);
    check_flag!(doc = "User/supervisor; user-mode accesses are not allowed to the 2-MByte region controlled by this entry.",
                is_user_mode_allowed, PD_US);
    check_flag!(
        doc = "Page-level write-through.",
        is_page_write_through,
        PD_PWT
    );
    check_flag!(
        doc = "Page-level cache disable.",
        is_page_level_cache_disabled,
        PD_PCD
    );
    check_flag!(doc = "Accessed; if PD_PS set indicates whether software has accessed the 2-MByte page else indicates whether this entry has been used for linear-address translation.",
                is_accessed, PD_A);
    check_flag!(doc = "Dirty; if PD_PS set indicates whether software has written to the 2-MByte page referenced by this entry else ignored.",
                is_dirty, PD_D);
    check_flag!(doc = "Page size; if set this entry maps a 2-MByte page; otherwise, this entry references a page directory.",
                is_page, PD_PS);
    check_flag!(doc = "Global; if PD_PS && CR4.PGE = 1, determines whether the translation is global; ignored otherwise if not PD_PS this is ignored.",
                is_global, PD_G);
    check_flag!(doc = "Indirectly determines the memory type used to access the 2-MByte page referenced by this entry. if not PD_PS this is ignored.",
                is_pat, PD_PAT);
    check_flag!(doc = "If IA32_EFER.NXE = 1, execute-disable. If 1, instruction fetches are not allowed from the 2-Mbyte region.",
                is_instruction_fetching_disabled, PD_XD);
}

bitflags! {
    /// PT Entry bits description.
    pub struct PTEntry: u64 {
        /// Present; must be 1 to map a 4-KByte page.
        const PT_P       = bit!(0);
        /// Read/write; if 0, writes may not be allowed to the 4-KByte region controlled by this entry
        const PT_RW      = bit!(1);
        /// User/supervisor; user-mode accesses are not allowed to the 4-KByte region controlled by this entry.
        const PT_US      = bit!(2);
        /// Page-level write-through.
        const PT_PWT     = bit!(3);
        /// Page-level cache disable.
        const PT_PCD     = bit!(4);
        /// Accessed; indicates whether software has accessed the 4-KByte page
        const PT_A       = bit!(5);
        /// Dirty; indicates whether software has written to the 4-KByte page referenced by this entry.
        const PT_D       = bit!(6);
        /// Global; if CR4.PGE = 1, determines whether the translation is global (see Section 4.10); ignored otherwise
        const PT_G       = bit!(8);
        /// If IA32_EFER.NXE = 1, execute-disable
        /// If 1, instruction fetches are not allowed from the 512-GByte region.
        const PT_XD      = bit!(63);
    }
}

impl PTEntry {
    /// Creates a new PTEntry.
    ///
    /// # Arguments
    ///
    ///  * `page` - The physical address of the backing 4 KiB page.
    ///  * `flags`- Additional flags for the entry.
    pub fn new(page: PAddr, flags: PTEntry) -> PTEntry {
        assert!(page.into(): usize % BASE_PAGE_LENGTH == 0);
        PTEntry {
            bits: page.into(): u64 | flags.bits,
        }
    }

    /// Retrieves the physical address in this entry.
    pub fn get_address(self) -> PAddr {
        PAddr::from(self.bits & ADDRESS_MASK)
    }

    check_flag!(
        doc = "Present; must be 1 to map a 4-KByte page or reference a page table.",
        is_present,
        PT_P
    );
    check_flag!(doc = "Read/write; if 0, writes may not be allowed to the 4-KByte region controlled by this entry",
                is_writeable, PT_RW);
    check_flag!(doc = "User/supervisor; user-mode accesses are not allowed to the 4-KByte region controlled by this entry.",
                is_user_mode_allowed, PT_US);
    check_flag!(
        doc = "Page-level write-through.",
        is_page_write_through,
        PT_PWT
    );
    check_flag!(
        doc = "Page-level cache disable.",
        is_page_level_cache_disabled,
        PT_PCD
    );
    check_flag!(doc = "Accessed; if PT_PS set indicates whether software has accessed the 4-KByte page else indicates whether this entry has been used for linear-address translation.",
                is_accessed, PT_A);
    check_flag!(doc = "Dirty; if PD_PS set indicates whether software has written to the 4-KByte page referenced by this entry else ignored.",
                is_dirty, PT_D);
    check_flag!(doc = "Global; if PT_PS && CR4.PGE = 1, determines whether the translation is global; ignored otherwise if not PT_PS this is ignored.",
                is_global, PT_G);
    check_flag!(doc = "If IA32_EFER.NXE = 1, execute-disable. If 1, instruction fetches are not allowed from the 4-KByte region.",
                is_instruction_fetching_disabled, PT_XD);
}
