//! This module contains the parallel iterator types for slices
//! (`[T]`). You will rarely need to interact with it directly unless
//! you have need to name one of those types.

use iter::*;
use iter::internal::*;
use std::borrow::{Borrow, BorrowMut};
use std::cmp;

/// Parallel extensions for slices.
pub trait ParallelSlice<T: Sync>: Borrow<[T]> {
    /// Returns a parallel iterator over subslices separated by elements that
    /// match the separator.
    fn par_split<P>(&self, separator: P) -> Split<T, P>
        where P: Fn(&T) -> bool + Sync
    {
        Split {
            slice: self.borrow(),
            separator: separator,
        }
    }

    /// Returns a parallel iterator over all contiguous windows of
    /// length `size`. The windows overlap.
    fn par_windows(&self, window_size: usize) -> Windows<T> {
        Windows {
            window_size: window_size,
            slice: self.borrow(),
        }
    }

    /// Returns a parallel iterator over at most `size` elements of
    /// `self` at a time. The chunks do not overlap.
    fn par_chunks(&self, chunk_size: usize) -> Chunks<T> {
        Chunks {
            chunk_size: chunk_size,
            slice: self.borrow(),
        }
    }
}

impl<T: Sync, V: ?Sized + Borrow<[T]>> ParallelSlice<T> for V {}


/// Parallel extensions for mutable slices.
pub trait ParallelSliceMut<T: Send>: BorrowMut<[T]> {
    /// Returns a parallel iterator over at most `size` elements of
    /// `self` at a time. The chunks are mutable and do not overlap.
    fn par_chunks_mut(&mut self, chunk_size: usize) -> ChunksMut<T> {
        ChunksMut {
            chunk_size: chunk_size,
            slice: self.borrow_mut(),
        }
    }
}

impl<T: Send, V: ?Sized + BorrowMut<[T]>> ParallelSliceMut<T> for V {}


impl<'data, T: Sync + 'data> IntoParallelIterator for &'data [T] {
    type Item = &'data T;
    type Iter = Iter<'data, T>;

    fn into_par_iter(self) -> Self::Iter {
        Iter { slice: self }
    }
}

impl<'data, T: Sync + 'data> IntoParallelIterator for &'data Vec<T> {
    type Item = &'data T;
    type Iter = Iter<'data, T>;

    fn into_par_iter(self) -> Self::Iter {
        Iter { slice: self }
    }
}

impl<'data, T: Send + 'data> IntoParallelIterator for &'data mut [T] {
    type Item = &'data mut T;
    type Iter = IterMut<'data, T>;

    fn into_par_iter(self) -> Self::Iter {
        IterMut { slice: self }
    }
}

impl<'data, T: Send + 'data> IntoParallelIterator for &'data mut Vec<T> {
    type Item = &'data mut T;
    type Iter = IterMut<'data, T>;

    fn into_par_iter(self) -> Self::Iter {
        IterMut { slice: self }
    }
}


/// Parallel iterator over immutable items in a slice
pub struct Iter<'data, T: 'data + Sync> {
    slice: &'data [T],
}

impl<'data, T: Sync + 'data> ParallelIterator for Iter<'data, T> {
    type Item = &'data T;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        bridge(self, consumer)
    }

    fn opt_len(&mut self) -> Option<usize> {
        Some(self.len())
    }
}

impl<'data, T: Sync + 'data> IndexedParallelIterator for Iter<'data, T> {
    fn drive<C>(self, consumer: C) -> C::Result
        where C: Consumer<Self::Item>
    {
        bridge(self, consumer)
    }

    fn len(&mut self) -> usize {
        self.slice.len()
    }

    fn with_producer<CB>(self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        callback.callback(IterProducer { slice: self.slice })
    }
}

struct IterProducer<'data, T: 'data + Sync> {
    slice: &'data [T],
}

impl<'data, T: 'data + Sync> Producer for IterProducer<'data, T> {
    type Item = &'data T;
    type IntoIter = ::std::slice::Iter<'data, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.slice.into_iter()
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.slice.split_at(index);
        (IterProducer { slice: left }, IterProducer { slice: right })
    }
}


/// Parallel iterator over immutable non-overlapping chunks of a slice
pub struct Chunks<'data, T: 'data + Sync> {
    chunk_size: usize,
    slice: &'data [T],
}

impl<'data, T: Sync + 'data> ParallelIterator for Chunks<'data, T> {
    type Item = &'data [T];

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        bridge(self, consumer)
    }

    fn opt_len(&mut self) -> Option<usize> {
        Some(self.len())
    }
}

impl<'data, T: Sync + 'data> IndexedParallelIterator for Chunks<'data, T> {
    fn drive<C>(self, consumer: C) -> C::Result
        where C: Consumer<Self::Item>
    {
        bridge(self, consumer)
    }

    fn len(&mut self) -> usize {
        (self.slice.len() + (self.chunk_size - 1)) / self.chunk_size
    }

    fn with_producer<CB>(self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        callback.callback(ChunksProducer {
                              chunk_size: self.chunk_size,
                              slice: self.slice,
                          })
    }
}

struct ChunksProducer<'data, T: 'data + Sync> {
    chunk_size: usize,
    slice: &'data [T],
}

impl<'data, T: 'data + Sync> Producer for ChunksProducer<'data, T> {
    type Item = &'data [T];
    type IntoIter = ::std::slice::Chunks<'data, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.slice.chunks(self.chunk_size)
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let elem_index = index * self.chunk_size;
        let (left, right) = self.slice.split_at(elem_index);
        (ChunksProducer {
             chunk_size: self.chunk_size,
             slice: left,
         },
         ChunksProducer {
             chunk_size: self.chunk_size,
             slice: right,
         })
    }
}


/// Parallel iterator over immutable overlapping windows of a slice
pub struct Windows<'data, T: 'data + Sync> {
    window_size: usize,
    slice: &'data [T],
}

impl<'data, T: Sync + 'data> ParallelIterator for Windows<'data, T> {
    type Item = &'data [T];

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        bridge(self, consumer)
    }

    fn opt_len(&mut self) -> Option<usize> {
        Some(self.len())
    }
}

impl<'data, T: Sync + 'data> IndexedParallelIterator for Windows<'data, T> {
    fn drive<C>(self, consumer: C) -> C::Result
        where C: Consumer<Self::Item>
    {
        bridge(self, consumer)
    }

    fn len(&mut self) -> usize {
        assert!(self.window_size >= 1);
        self.slice.len().saturating_sub(self.window_size - 1)
    }

    fn with_producer<CB>(self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        callback.callback(WindowsProducer {
                              window_size: self.window_size,
                              slice: self.slice,
                          })
    }
}

struct WindowsProducer<'data, T: 'data + Sync> {
    window_size: usize,
    slice: &'data [T],
}

impl<'data, T: 'data + Sync> Producer for WindowsProducer<'data, T> {
    type Item = &'data [T];
    type IntoIter = ::std::slice::Windows<'data, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.slice.windows(self.window_size)
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let left_index = cmp::min(self.slice.len(), index + (self.window_size - 1));
        let left = &self.slice[..left_index];
        let right = &self.slice[index..];
        (WindowsProducer {
             window_size: self.window_size,
             slice: left,
         },
         WindowsProducer {
             window_size: self.window_size,
             slice: right,
         })
    }
}


/// Parallel iterator over mutable items in a slice
pub struct IterMut<'data, T: 'data + Send> {
    slice: &'data mut [T],
}

impl<'data, T: Send + 'data> ParallelIterator for IterMut<'data, T> {
    type Item = &'data mut T;

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        bridge(self, consumer)
    }

    fn opt_len(&mut self) -> Option<usize> {
        Some(self.len())
    }
}

impl<'data, T: Send + 'data> IndexedParallelIterator for IterMut<'data, T> {
    fn drive<C>(self, consumer: C) -> C::Result
        where C: Consumer<Self::Item>
    {
        bridge(self, consumer)
    }

    fn len(&mut self) -> usize {
        self.slice.len()
    }

    fn with_producer<CB>(self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        callback.callback(IterMutProducer { slice: self.slice })
    }
}

struct IterMutProducer<'data, T: 'data + Send> {
    slice: &'data mut [T],
}

impl<'data, T: 'data + Send> Producer for IterMutProducer<'data, T> {
    type Item = &'data mut T;
    type IntoIter = ::std::slice::IterMut<'data, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.slice.into_iter()
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let (left, right) = self.slice.split_at_mut(index);
        (IterMutProducer { slice: left }, IterMutProducer { slice: right })
    }
}


/// Parallel iterator over mutable non-overlapping chunks of a slice
pub struct ChunksMut<'data, T: 'data + Send> {
    chunk_size: usize,
    slice: &'data mut [T],
}

impl<'data, T: Send + 'data> ParallelIterator for ChunksMut<'data, T> {
    type Item = &'data mut [T];

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        bridge(self, consumer)
    }

    fn opt_len(&mut self) -> Option<usize> {
        Some(self.len())
    }
}

impl<'data, T: Send + 'data> IndexedParallelIterator for ChunksMut<'data, T> {
    fn drive<C>(self, consumer: C) -> C::Result
        where C: Consumer<Self::Item>
    {
        bridge(self, consumer)
    }

    fn len(&mut self) -> usize {
        (self.slice.len() + (self.chunk_size - 1)) / self.chunk_size
    }

    fn with_producer<CB>(self, callback: CB) -> CB::Output
        where CB: ProducerCallback<Self::Item>
    {
        callback.callback(ChunksMutProducer {
                              chunk_size: self.chunk_size,
                              slice: self.slice,
                          })
    }
}

struct ChunksMutProducer<'data, T: 'data + Send> {
    chunk_size: usize,
    slice: &'data mut [T],
}

impl<'data, T: 'data + Send> Producer for ChunksMutProducer<'data, T> {
    type Item = &'data mut [T];
    type IntoIter = ::std::slice::ChunksMut<'data, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.slice.chunks_mut(self.chunk_size)
    }

    fn split_at(self, index: usize) -> (Self, Self) {
        let elem_index = index * self.chunk_size;
        let (left, right) = self.slice.split_at_mut(elem_index);
        (ChunksMutProducer {
             chunk_size: self.chunk_size,
             slice: left,
         },
         ChunksMutProducer {
             chunk_size: self.chunk_size,
             slice: right,
         })
    }
}


/// Parallel iterator over slices separated by a predicate
pub struct Split<'data, T: 'data, P> {
    slice: &'data [T],
    separator: P,
}

struct SplitProducer<'data, 'sep, T: 'data, P: 'sep> {
    slice: &'data [T],
    separator: &'sep P,

    /// Marks the endpoint beyond which we've already found no separators.
    tail: usize,
}

impl<'data, T, P> ParallelIterator for Split<'data, T, P>
    where P: Fn(&T) -> bool + Sync,
          T: Sync
{
    type Item = &'data [T];

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
        where C: UnindexedConsumer<Self::Item>
    {
        let producer = SplitProducer {
            slice: self.slice,
            separator: &self.separator,
            tail: self.slice.len(),
        };
        bridge_unindexed(producer, consumer)
    }
}

impl<'data, 'sep, T, P> UnindexedProducer for SplitProducer<'data, 'sep, T, P>
    where P: Fn(&T) -> bool + Sync,
          T: Sync
{
    type Item = &'data [T];

    fn split(mut self) -> (Self, Option<Self>) {
        let SplitProducer { slice, separator, tail } = self;

        // Look forward for the separator, and failing that look backward.
        let mid = tail / 2;
        let index = slice[mid..tail].iter().position(separator)
            .map(|i| mid + i)
            .or_else(|| slice[..mid].iter().rposition(separator));

        if let Some(index) = index {
            let (left, right) = slice.split_at(index);

            // Update `self` as the region before the separator.
            self.slice = left;
            self.tail = cmp::min(mid, index);

            // Create the right split following the separator.
            let mut right = SplitProducer {
                slice: &right[1..],
                separator: separator,
                tail: tail - index - 1,
            };

            // If we scanned backwards to find the separator, everything in
            // the right side is exhausted, with no separators left to find.
            if index < mid {
                right.tail = 0;
            }

            (self, Some(right))

        } else {
            self.tail = 0;
            (self, None)
        }
    }

    fn fold_with<F>(self, folder: F) -> F
        where F: Folder<Self::Item>
    {
        let SplitProducer { slice, separator, tail } = self;

        if tail == slice.len() {
            // No tail section, so just let `slice::split` handle it.
            folder.consume_iter(slice.split(separator))

        } else if let Some(index) = slice[..tail].iter().rposition(separator) {
            // We found the last separator to complete the tail, so
            // end with that slice after `slice::split` finds the rest.
            let (left, right) = slice.split_at(index);
            let folder = folder.consume_iter(left.split(separator));
            if folder.full() {
                folder
            } else {
                // skip the separator
                folder.consume(&right[1..])
            }

        } else {
            // We know there are no separators at all.  Return our whole slice.
            folder.consume(slice)
        }
    }
}
