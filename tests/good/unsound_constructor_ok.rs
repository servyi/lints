#![feature(register_tool)]
#![register_tool(servyi)]

/// A lease over one child.
///
/// SAFETY (constructor): the struct literal aliases the storage of the
/// `&mut` it is built from; sound only when exactly one handle exists per
/// disjoint item — construction sites must prove exactly that.
#[servyi::unsound_constructor]
pub struct Handle<'o, T> {
    ptr: *mut T,
    _b: std::marker::PhantomData<&'o ()>,
}

fn doc_above(v: &mut u8) -> Handle<'_, u8> {
    // SAFETY: `v` is a fresh exclusive borrow; this is its only handle.
    Handle { ptr: v as *mut u8, _b: Default::default() }
}

fn block_style(v: &mut u8) -> Handle<'_, u8> {
    /* SAFETY: block comments count too; `v` is exclusive and fresh. */
    Handle { ptr: v as *mut u8, _b: Default::default() }
}

fn inside_braces(v: &mut u8) -> Handle<'_, u8> {
    Handle {
        // SAFETY: comments inside the literal's braces, next to the
        // fields, also satisfy the requirement.
        ptr: v as *mut u8,
        _b: Default::default(),
    }
}

const INITIAL: u8 = 0;

fn main() {
    let mut v = INITIAL;
    let _ = doc_above(&mut v);
    let _ = block_style(&mut v);
    let _ = inside_braces(&mut v);
}
