//! Helper for building a temporary `alpm_list_t` chain of *borrowed*
//! payload pointers to pass into libalpm functions that take an input
//! list (`alpm_db_search`, `alpm_db_update`, `alpm_checkdeps`,
//! `alpm_checkconflicts`, `alpm_find_satisfier`, ...).
//!
//! # Ownership
//! The chain *nodes* this builds are heap-allocated (`Box`ed) and owned
//! by the returned [`BorrowedList`]; they are freed automatically when
//! it is dropped (plain `Vec`/`Box` deallocation — never
//! `alpm_list_free`, since libalpm did not allocate these nodes). The
//! `data` payload each node points at is supplied by the caller and
//! must already be valid for at least as long as the `BorrowedList`
//! lives; this helper never takes ownership of the payloads.

use crate::sys;
use std::marker::PhantomData;
use std::ptr;

pub(crate) struct BorrowedList<'a> {
    // Kept alive so the pointers each node's `next` refers to, and the
    // head pointer handed to libalpm, stay valid for 'a.
    _nodes: Vec<Box<sys::alpm_list_t>>,
    head: *mut sys::alpm_list_t,
    _marker: PhantomData<&'a ()>,
}

impl<'a> BorrowedList<'a> {
    /// Build a chain whose node `data` payloads are exactly the given
    /// raw pointers, in order.
    pub(crate) fn from_ptrs(ptrs: impl IntoIterator<Item = *mut std::os::raw::c_void>) -> Self {
        let mut nodes: Vec<Box<sys::alpm_list_t>> = ptrs
            .into_iter()
            .map(|data| {
                Box::new(sys::alpm_list_t {
                    data,
                    prev: ptr::null_mut(),
                    next: ptr::null_mut(),
                })
            })
            .collect();

        for i in 0..nodes.len() {
            let next_ptr = if i + 1 < nodes.len() {
                &mut *nodes[i + 1] as *mut sys::alpm_list_t
            } else {
                ptr::null_mut()
            };
            nodes[i].next = next_ptr;
        }
        let head = nodes
            .first_mut()
            .map(|b| &mut **b as *mut sys::alpm_list_t)
            .unwrap_or(ptr::null_mut());

        Self {
            _nodes: nodes,
            head,
            _marker: PhantomData,
        }
    }

    pub(crate) fn as_raw(&self) -> *mut sys::alpm_list_t {
        self.head
    }
}
