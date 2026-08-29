//! Safe iteration over libalpm's `alpm_list_t` doubly-linked list.
//!
//! `alpm_list_t` is a public, front-end-facing list type (see
//! `alpm_list.h`'s own doc comment: "exposed so front ends can use it").
//! Nyx never allocates or frees list *nodes* itself — every list handed to
//! us either:
//!
//! 1. is **borrowed** from a libalpm-owned collection (e.g.
//!    `alpm_db_get_pkgcache`, `alpm_pkg_get_depends`) and must not be
//!    freed by us at all — libalpm owns it for the lifetime of the handle
//!    it was gotten from; or
//! 2. is a list *we* allocated by calling an ALPM function that returns
//!    ownership to the caller (e.g. `alpm_checkdeps`), in which case the
//!    specific ALPM `_free` function for that element type must be used
//!    (never a generic `alpm_list_free`, which would leak/double-free the
//!    element payloads for anything but a list of scalars).
//!
//! This module only provides read-only iteration (`AlpmList::iter`); it
//! never frees anything. Freeing owned lists is the responsibility of the
//! specific wrapper that allocated them (see `resolve.rs`), right next to
//! the call that produced the list, so the free obviously matches the
//! allocation.

use crate::sys;
use std::marker::PhantomData;

/// A borrowed view over an `alpm_list_t*` whose node payloads are `*mut T`.
///
/// # Safety invariant
/// The caller must guarantee `head` is either null or points to a valid,
/// libalpm-owned `alpm_list_t` chain whose `data` pointers are valid
/// `*mut T` for at least as long as `'a`, and that nothing mutates/frees
/// that chain while this `AlpmList` is alive. This holds for every list
/// libalpm hands back through the accessor functions nyx-alpm calls,
/// which document that the returned list stays valid until the owning
/// handle/package/db is released.
pub struct AlpmList<'a, T> {
    head: *mut sys::alpm_list_t,
    _marker: PhantomData<&'a T>,
}

impl<'a, T> AlpmList<'a, T> {
    /// # Safety
    /// See the struct-level invariant above.
    pub(crate) unsafe fn from_raw(head: *mut sys::alpm_list_t) -> Self {
        Self {
            head,
            _marker: PhantomData,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.head.is_null()
    }

    pub fn len(&self) -> usize {
        unsafe { sys::alpm_list_count(self.head) }
    }

    pub fn iter(&self) -> AlpmListIter<'a, T> {
        AlpmListIter {
            node: self.head,
            _marker: PhantomData,
        }
    }
}

pub struct AlpmListIter<'a, T> {
    node: *mut sys::alpm_list_t,
    _marker: PhantomData<&'a T>,
}

impl<'a, T> Iterator for AlpmListIter<'a, T> {
    type Item = *mut T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.node.is_null() {
            return None;
        }
        // SAFETY: `self.node` is non-null and, by the AlpmList invariant,
        // points at a valid alpm_list_t node whose `data` field is a valid
        // `*mut T` for the list's lifetime. Dereferencing to read `data`
        // and `next` does not mutate the list.
        let (data, next) = unsafe { ((*self.node).data as *mut T, (*self.node).next) };
        self.node = next;
        Some(data)
    }
}

impl<'a, T> IntoIterator for &'_ AlpmList<'a, T> {
    type Item = *mut T;
    type IntoIter = AlpmListIter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Exercise the iteration logic against a hand-built alpm_list_t chain
    // (three nodes holding raw i32 pointers) without needing libalpm at
    // all, since alpm_list_t's shape is a fixed, documented public struct
    // independent of the rest of the library.
    #[test]
    fn iterates_in_order_over_manually_built_chain() {
        let mut a = 1i32;
        let mut b = 2i32;
        let mut c = 3i32;

        let mut n2 = sys::alpm_list_t {
            data: &mut c as *mut i32 as *mut _,
            prev: std::ptr::null_mut(),
            next: std::ptr::null_mut(),
        };
        let mut n1 = sys::alpm_list_t {
            data: &mut b as *mut i32 as *mut _,
            prev: std::ptr::null_mut(),
            next: &mut n2 as *mut _,
        };
        let n0 = sys::alpm_list_t {
            data: &mut a as *mut i32 as *mut _,
            prev: std::ptr::null_mut(),
            next: &mut n1 as *mut _,
        };
        let mut n0 = n0;

        let list: AlpmList<i32> = unsafe { AlpmList::from_raw(&mut n0 as *mut _) };
        assert_eq!(list.len(), 3);
        let values: Vec<i32> = list.iter().map(|p| unsafe { *p }).collect();
        assert_eq!(values, vec![1, 2, 3]);
    }

    #[test]
    fn empty_list_iterates_zero_times() {
        let list: AlpmList<i32> = unsafe { AlpmList::from_raw(std::ptr::null_mut()) };
        assert!(list.is_empty());
        assert_eq!(list.len(), 0);
        assert_eq!(list.iter().count(), 0);
    }
}
