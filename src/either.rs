use bevy::{
    ecs::{
        Fetch,
        WorldQuery,
        ReadOnlyFetch,
        QueryAccess,
        Archetype,
    },
    prelude::*,
};

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum Either<T, U> {
    Left(T),
    Right(U),
}

impl<T: WorldQuery, U: WorldQuery> WorldQuery for Either<T, U> {
    type Fetch = EitherFetch<T::Fetch, U::Fetch>;
}

pub struct EitherFetch<T, U>(Either<T, U>);

unsafe impl<T, U> ReadOnlyFetch for EitherFetch<T, U> where T: ReadOnlyFetch, U: ReadOnlyFetch {}

impl<'a, T: Fetch<'a>, U: Fetch<'a>> Fetch<'a> for EitherFetch<T, U> {
    type Item = Either<T::Item, U::Item>;

    const DANGLING: Self = Self(Either::Right(U::DANGLING));

    #[inline]
    fn access() -> QueryAccess {
        QueryAccess::union(vec![QueryAccess::optional(T::access()), QueryAccess::optional(U::access())])
    }

    unsafe fn get(archetype: &'a Archetype, offset: usize) -> Option<Self> {
        match T::get(archetype, offset) {
            Some(s) => Some(EitherFetch(Either::Left(s))),
            None => U::get(archetype, offset).map(Either::Right).map(EitherFetch)
        }
    }

    unsafe fn fetch(&self, n: usize) -> Self::Item {
        // /shrug
    }
}