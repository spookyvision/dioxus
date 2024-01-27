use generational_box::GenerationalBoxId;
use generational_box::UnsyncStorage;
use std::mem::MaybeUninit;
use std::ops::Deref;
use std::rc::Rc;

use dioxus_core::prelude::*;
use dioxus_core::ScopeId;

use generational_box::{GenerationalBox, Owner, Storage};

// use crate::Effect;
// use crate::Readable;
// use crate::Writable;

fn current_owner<S: Storage<T>, T>() -> Rc<Owner<S>> {
    match Effect::current() {
        // If we are inside of an effect, we should use the owner of the effect as the owner of the value.
        Some(effect) => {
            let scope_id = effect.source;
            owner_in_scope(scope_id)
        }
        // Otherwise either get an owner from the current scope or create a new one.
        None => match has_context() {
            Some(rt) => rt,
            None => {
                let owner = Rc::new(S::owner());
                provide_context(owner)
            }
        },
    }
}

fn owner_in_scope<S: Storage<T>, T>(scope: ScopeId) -> Rc<Owner<S>> {
    match consume_context_from_scope(scope) {
        Some(rt) => rt,
        None => {
            let owner = Rc::new(S::owner());
            scope.provide_context(owner)
        }
    }
}

/// CopyValue is a wrapper around a value to make the value mutable and Copy.
///
/// It is internally backed by [`generational_box::GenerationalBox`].
pub struct CopyValue<T: ?Sized + 'static, S: 'static = UnsyncStorage> {
    pub(crate) value: GenerationalBox<T, S>,
    origin_scope: ScopeId,
}

#[test]
fn it_copies() {
    fn makes() -> CopyValue<dyn Fn()> {
        todo!()
    }

    let g = makes();

    g.clone();
}

#[cfg(feature = "serde")]
impl<T: 'static, Store: Storage<T>> serde::Serialize for CopyValue<T, Store>
where
    T: serde::Serialize,
{
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.value.read().serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de, T: 'static, Store: Storage<T>> serde::Deserialize<'de> for CopyValue<T, Store>
where
    T: serde::Deserialize<'de>,
{
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = T::deserialize(deserializer)?;

        Ok(Self::new_maybe_sync(value))
    }
}

impl<T: 'static + ?Sized> CopyValue<T> {
    /// Create a new CopyValue. The value will be stored in the current component.
    ///
    /// Once the component this value is created in is dropped, the value will be dropped.
    #[track_caller]
    pub fn new(value: Box<T>) -> Self {
        Self::new_maybe_sync(value)
    }

    /// Create a new CopyValue. The value will be stored in the given scope. When the specified scope is dropped, the value will be dropped.
    #[track_caller]
    pub fn new_in_scope(value: Box<T>, scope: ScopeId) -> Self {
        Self::new_maybe_sync_in_scope(value, scope)
    }
}

impl<T: 'static + ?Sized, S: Storage<T>> CopyValue<T, S> {
    /// Create a new CopyValue. The value will be stored in the current component.
    ///
    /// Once the component this value is created in is dropped, the value will be dropped.
    #[track_caller]
    pub fn new_maybe_sync(value: Box<T>) -> Self {
        let owner = current_owner();

        Self {
            value: owner.insert(value),
            origin_scope: current_scope_id().expect("in a virtual dom"),
        }
    }

    pub(crate) fn new_with_caller(
        value: Box<T>,
        #[cfg(debug_assertions)] caller: &'static std::panic::Location<'static>,
    ) -> Self {
        let owner = current_owner();

        Self {
            value: owner.insert_with_caller(
                value,
                #[cfg(debug_assertions)]
                caller,
            ),
            origin_scope: current_scope_id().expect("in a virtual dom"),
        }
    }

    /// Create a new CopyValue. The value will be stored in the given scope. When the specified scope is dropped, the value will be dropped.
    #[track_caller]
    pub fn new_maybe_sync_in_scope(value: Box<T>, scope: ScopeId) -> Self {
        let owner = owner_in_scope(scope);

        Self {
            value: owner.insert(value),
            origin_scope: scope,
        }
    }

    pub(crate) fn invalid() -> Self {
        let owner = current_owner();

        Self {
            value: owner.invalid(),
            origin_scope: current_scope_id().expect("in a virtual dom"),
        }
    }

    /// Get the scope this value was created in.
    pub fn origin_scope(&self) -> ScopeId {
        self.origin_scope
    }

    /// Get the generational id of the value.
    pub fn id(&self) -> GenerationalBoxId {
        self.value.id()
    }
}

impl<T: 'static + ?Sized, S: Storage<T>> Readable<T> for CopyValue<T, S> {
    type Ref<R: ?Sized + 'static> = S::Ref<R>;

    fn map_ref<I: ?Sized, U: ?Sized, F: FnOnce(&I) -> &U>(
        ref_: Self::Ref<I>,
        f: F,
    ) -> Self::Ref<U> {
        S::map(ref_, f)
    }

    fn try_map_ref<I, U: ?Sized, F: FnOnce(&I) -> Option<&U>>(
        ref_: Self::Ref<I>,
        f: F,
    ) -> Option<Self::Ref<U>> {
        S::try_map(ref_, f)
    }

    fn read(&self) -> Self::Ref<T> {
        self.value.read()
    }

    fn peek(&self) -> Self::Ref<T> {
        self.value.read()
    }
}

impl<T: 'static + ?Sized, S: Storage<T>> Writable<T> for CopyValue<T, S> {
    type Mut<R: ?Sized + 'static> = S::Mut<R>;

    fn map_mut<I, U: ?Sized, F: FnOnce(&mut I) -> &mut U>(
        mut_: Self::Mut<I>,
        f: F,
    ) -> Self::Mut<U> {
        S::map_mut(mut_, f)
    }

    fn try_map_mut<I, U: ?Sized, F: FnOnce(&mut I) -> Option<&mut U>>(
        mut_: Self::Mut<I>,
        f: F,
    ) -> Option<Self::Mut<U>> {
        S::try_map_mut(mut_, f)
    }

    fn try_write(&self) -> Result<Self::Mut<T>, generational_box::BorrowMutError> {
        self.value.try_write()
    }

    fn write(&self) -> Self::Mut<T> {
        self.value.write()
    }

    fn set(&mut self, value: T) {
        if let Ok(mut val) = self.try_write() {
            *val = value;
        } else {
            self.value.set(Box::new(value));
        }
    }
}

impl<T: 'static + ?Sized, S: Storage<T>> PartialEq for CopyValue<T, S> {
    fn eq(&self, other: &Self) -> bool {
        self.value.ptr_eq(&other.value)
    }
}

impl<T: 'static + Copy + ?Sized, S: Storage<T>> Deref for CopyValue<T, S> {
    type Target = dyn Fn() -> T;

    fn deref(&self) -> &Self::Target {
        // https://github.com/dtolnay/case-studies/tree/master/callable-types

        // First we create a closure that captures something with the Same in memory layout as Self (MaybeUninit<Self>).
        let uninit_callable = MaybeUninit::<Self>::uninit();
        // Then move that value into the closure. We assume that the closure now has a in memory layout of Self.
        let uninit_closure = move || *Self::read(unsafe { &*uninit_callable.as_ptr() });

        // Check that the size of the closure is the same as the size of Self in case the compiler changed the layout of the closure.
        let size_of_closure = std::mem::size_of_val(&uninit_closure);
        assert_eq!(size_of_closure, std::mem::size_of::<Self>());

        // Then cast the lifetime of the closure to the lifetime of &self.
        fn cast_lifetime<'a, T>(_a: &T, b: &'a T) -> &'a T {
            b
        }
        let reference_to_closure = cast_lifetime(
            {
                // The real closure that we will never use.
                &uninit_closure
            },
            // We transmute self into a reference to the closure. This is safe because we know that the closure has the same memory layout as Self so &Closure == &Self.
            unsafe { std::mem::transmute(self) },
        );

        // Cast the closure to a trait object.
        reference_to_closure as &Self::Target
    }
}