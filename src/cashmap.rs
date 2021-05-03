#![allow(dead_code)]
use std::any::type_name;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

use serde::{de::DeserializeOwned, Deserialize, Serialize};

use cosmwasm_std::{ReadonlyStorage, StdError, StdResult, Storage};

use cosmwasm_storage::{PrefixedStorage, ReadonlyPrefixedStorage};
use secret_toolkit::serialization::{Bincode2, Serde};
use std::cmp::min;
use std::collections::hash_map::DefaultHasher;

const INDEXES: &[u8] = b"indexes";
const MAP_LENGTH: &[u8] = b"length";

const PAGE_SIZE: u32 = 100;

fn _page_from_position(position: u32) -> u32 {
    return position / PAGE_SIZE;
}

#[derive(Serialize, Deserialize, Copy, Clone)]
struct MetaData {
    position: u32,
}

#[derive(Serialize, Deserialize)]
pub struct InternalItem<T>
// where
//     T: Serialize + DeserializeOwned,
{
    item: T,
    meta_data: MetaData,
}

pub struct CashMap<'a, T, S, Ser = Bincode2>
where
    T: Serialize + DeserializeOwned,
    S: Storage,
    Ser: Serde,
{
    storage: &'a mut S,
    item_type: PhantomData<*const InternalItem<T>>,
    serialization_type: PhantomData<*const Ser>,
    prefix: Option<Vec<u8>>,
}

impl<'a, T, S> CashMap<'a, T, S, Bincode2>
where
    T: Serialize + DeserializeOwned,
    S: Storage,
{
    pub fn init(name: &[u8], storage: &'a mut S) -> Self {
        Self::attach_with_serialization(storage, Bincode2, Some(name.to_vec()))
    }

    pub fn attach(storage: &'a mut S) -> Self {
        Self::attach_with_serialization(storage, Bincode2, None)
    }
}

impl<'a, T, S, Ser> CashMap<'a, T, S, Ser>
where
    T: Serialize + DeserializeOwned,
    // K: Hash + Eq + ?Sized,
    S: Storage,
    Ser: Serde,
{
    pub fn len(&self) -> u32 {
        self.as_readonly().len()
    }

    /// This method allows customization of the serialization, in case we want to force
    /// something other than Bincode2, which has it's drawbacks (such as Enums fucking up)
    pub fn attach_with_serialization(
        storage: &'a mut S,
        _serialization: Ser,
        prefix: Option<Vec<u8>>,
    ) -> Self {
        Self {
            storage,
            serialization_type: PhantomData,
            item_type: PhantomData,
            prefix,
        }
    }

    pub fn remove(&mut self, key: &[u8]) -> StdResult<()> {
        let item = self.as_readonly()._direct_get(&key);

        if item.is_none() {
            return Err(StdError::not_found("Item not found in map"));
        }

        let removed_pos = item.unwrap().meta_data.position;

        let page = _page_from_position(removed_pos);

        let mut indexes = self.as_readonly().get_indexes(page);
        let hash = self.as_readonly().key_to_hash(key);

        let mut len = self.as_readonly().len();
        len -= 1;
        self.set_length(len)?;

        return if !indexes.contains(&hash) {
            Err(StdError::generic_err(
                "Tried to remove, but hash not found - should never happen",
            ))
        } else {
            if len == 0 || len == removed_pos {
                indexes.pop();
                self.store_indexes(page, &indexes)?;
                return Ok(self.remove_from_store(&hash.to_be_bytes()));
            }

            let pos_in_indexes = indexes.iter().position(|index| index == &hash).unwrap();

            // replace the last item with our new item

            let max_page = _page_from_position(len);

            let mut last_item_indexes = self.as_readonly().get_indexes(max_page);

            if let Some(last_item_hash) = last_item_indexes.pop() {
                self.store_indexes(max_page, &last_item_indexes)?;

                if self.as_readonly().get_no_hash(&last_item_hash).is_none() {
                    return Err(StdError::not_found("Failed to remove item from map"));
                }

                let mut last_item = self.as_readonly().get_no_hash(&last_item_hash).unwrap();

                last_item.meta_data.position = removed_pos;

                let _ = std::mem::replace(&mut indexes[pos_in_indexes], last_item_hash.clone());

                self.store(&last_item_hash.to_be_bytes(), &last_item)?;

                self.store_indexes(page, &indexes)?;
                Ok(self.remove_from_store(&hash.to_be_bytes()))
            } else {
                Err(StdError::not_found("Failed to remove item from map"))
            }
        };
    }

    pub fn insert(&mut self, key: &[u8], item: T) -> StdResult<()> {
        let hash = self.as_readonly().key_to_hash(key);

        if self.as_readonly().contains_key(key) {
            let position = self.get_position(key).unwrap();
            let to_store = InternalItem {
                item,
                meta_data: MetaData { position },
            };

            self.store(&hash.to_be_bytes(), &to_store)?;
        } else {
            let pos = self.len();
            let page = _page_from_position(pos);
            let mut indexes = self.as_readonly().get_indexes(page);

            if !indexes.contains(&hash) {
                indexes.push(hash);
                self.store_indexes(page, &indexes)?;
            }

            let to_store = InternalItem {
                item,
                meta_data: MetaData { position: pos },
            };
            self.store(&hash.to_be_bytes(), &to_store)?;
        }

        self.set_length(self.len() + 1)
    }

    /// user facing method to get T
    pub fn get(&self, key: &[u8]) -> Option<T> {
        self.as_readonly().get(key)
    }

    pub fn paging(&self, start_page: u32, size: u32) -> StdResult<Vec<T>> {
        self.as_readonly().paging(start_page, size)
    }

    pub fn contains(&self, key: &[u8]) -> bool {
        self.as_readonly().contains_key(key)
    }

    fn get_position(&self, key: &[u8]) -> Option<u32> {
        return if let Some(res) = self.as_readonly()._direct_get(&key) {
            Some(res.meta_data.position)
        } else {
            None
        };
    }

    fn store_indexes(&mut self, index: u32, indexes: &Vec<u64>) -> StdResult<()> {
        if let Some(prefix) = &self.prefix {
            let mut store = PrefixedStorage::new(prefix, self.storage);
            store.set(
                &[&INDEXES, index.to_be_bytes().to_vec().as_slice()].concat(),
                &Ser::serialize(indexes)?,
            );
        } else {
            self.storage.set(
                &[&INDEXES, index.to_be_bytes().to_vec().as_slice()].concat(),
                &Ser::serialize(indexes)?,
            );
        }
        Ok(())
    }

    fn remove_from_store(&mut self, key: &[u8]) {
        return if let Some(prefix) = &self.prefix {
            let mut store = PrefixedStorage::new(prefix, self.storage);
            store.remove(key)
        } else {
            self.storage.remove(key)
        };
    }

    fn store(&mut self, key: &[u8], item: &InternalItem<T>) -> StdResult<()> {
        if let Some(prefix) = &self.prefix {
            let mut store = PrefixedStorage::new(prefix, self.storage);
            store.set(key, &Ser::serialize(item)?)
        } else {
            self.storage.set(key, &Ser::serialize(item)?)
        }

        Ok(())
    }

    fn as_readonly(&self) -> ReadOnlyCashMap<T, S, Ser> {
        ReadOnlyCashMap {
            storage: self.storage,
            item_type: self.item_type,
            serialization_type: self.serialization_type,
            prefix: self.prefix.clone(),
        }
    }

    fn set_length(&mut self, length: u32) -> StdResult<()> {
        self.storage
            .set(MAP_LENGTH, &Ser::serialize(&length.to_be_bytes())?);
        Ok(())
    }

    fn load(&self, key: &[u8]) -> StdResult<T> {
        self.as_readonly().load(key)
    }
}

/// basically this is used in queries
pub struct ReadOnlyCashMap<'a, T, S, Ser = Bincode2>
where
    T: Serialize + DeserializeOwned,
    S: ReadonlyStorage,
    Ser: Serde,
{
    storage: &'a S,
    item_type: PhantomData<*const InternalItem<T>>,
    serialization_type: PhantomData<*const Ser>,
    prefix: Option<Vec<u8>>,
}

impl<'a, T, S> ReadOnlyCashMap<'a, T, S, Bincode2>
where
    T: Serialize + DeserializeOwned,
    S: ReadonlyStorage,
{
    pub fn init(name: &[u8], storage: &'a S) -> Self {
        Self::attach_with_serialization(storage, Bincode2, Some(name.to_vec()))
    }

    pub fn attach(storage: &'a S) -> Self {
        Self::attach_with_serialization(storage, Bincode2, None)
    }
}

impl<'a, T, S, Ser> ReadOnlyCashMap<'a, T, S, Ser>
where
    T: Serialize + DeserializeOwned,
    S: ReadonlyStorage,
    Ser: Serde,
{
    pub fn attach_with_serialization(
        storage: &'a S,
        _serialization: Ser,
        prefix: Option<Vec<u8>>,
    ) -> Self {
        Self {
            storage,
            serialization_type: PhantomData,
            item_type: PhantomData,
            prefix,
        }
    }

    pub fn contains_key(&self, key: &[u8]) -> bool {
        return self._direct_get(&key).is_some();
    }

    /// user facing method to get T
    pub fn get(&self, key: &[u8]) -> Option<T> {
        let hash = self.key_to_hash(key);

        if !(self.contains_key(key)) {
            return None;
        }

        return if let Ok(result) = self._direct_load(&hash) {
            Some(result)
        } else {
            None
        };
    }

    pub fn len(&self) -> u32 {
        let maybe_serialized = self.storage.get(&MAP_LENGTH);
        let serialized = maybe_serialized.unwrap_or_default();
        u32::from_be(Ser::deserialize(&serialized).unwrap_or_default())
    }

    /// starts from page 0
    ///
    /// Will return error if you access out of bounds
    pub fn paging(&self, start_page: u32, size: u32) -> StdResult<Vec<T>> {
        let start_pos = start_page * size;
        let mut end_pos = start_pos + size - 1;

        let max_size = self.len();

        if start_pos > max_size {
            return Err(StdError::NotFound {
                kind: "Out of bounds".to_string(),
                backtrace: None,
            });
        } else if end_pos > max_size {
            end_pos = max_size;
        }

        self.get_positions(start_pos, end_pos)
    }

    fn get_positions(&self, start: u32, end: u32) -> StdResult<Vec<T>> {
        let start_page = _page_from_position(start);
        let end_page = _page_from_position(end);

        let mut results = vec![];

        for page in start_page..=end_page {
            let start_pos = if page == start_page {
                start % PAGE_SIZE
            } else {
                0
            };

            //let start_pos = page * PAGE_SIZE;
            let max_page_pos = min(end, ((page + 1) * PAGE_SIZE) - 1) % PAGE_SIZE;

            let indexes = self.get_indexes(page);

            if max_page_pos as usize > indexes.len() {
                return Err(StdError::generic_err("Out of bounds"));
            }

            let hashes: Vec<u64> = indexes[start_pos as usize..=max_page_pos as usize].to_vec();

            let res: Vec<T> = hashes
                .iter()
                .map(|h| self._direct_load(h).unwrap())
                .collect();

            results.extend(res);
        }

        Ok(results)
    }

    fn get_indexes(&self, index: u32) -> Vec<u64> {
        let maybe_serialized = if let Some(prefix) = &self.prefix {
            let store = ReadonlyPrefixedStorage::new(prefix, self.storage);
            store.get(&[&INDEXES, index.to_be_bytes().to_vec().as_slice()].concat())
        } else {
            self.storage
                .get(&[&INDEXES, index.to_be_bytes().to_vec().as_slice()].concat())
        };
        let serialized = maybe_serialized.unwrap_or_default();
        Ser::deserialize(&serialized).unwrap_or_default()
    }

    fn _direct_load(&self, hash: &u64) -> StdResult<T> {
        let int_item = self._load_internal(hash)?;
        Ok(int_item.item)
    }

    /// get InternalItem and not just T
    fn _direct_get(&self, key: &[u8]) -> Option<InternalItem<T>> {
        let hash = self.key_to_hash(key);
        self.get_no_hash(&hash)
    }

    fn _load_internal(&self, hash: &u64) -> StdResult<InternalItem<T>> {
        let int_item = self._prefix_load(hash)?;
        Ok(int_item)
    }

    pub fn load(&self, key: &[u8]) -> StdResult<T> {
        let hash = self.key_to_hash(key);

        let int_item = self._prefix_load(&hash)?;
        Ok(int_item.item)
    }

    fn _prefix_load(&self, hash: &u64) -> StdResult<InternalItem<T>> {
        let serialized = if let Some(prefix) = &self.prefix {
            let store = ReadonlyPrefixedStorage::new(prefix, self.storage);
            store.get(&hash.to_be_bytes())
        } else {
            self.storage.get(&hash.to_be_bytes())
        }
        .ok_or_else(|| StdError::not_found(type_name::<T>()))?;

        let int_item: InternalItem<T> = Ser::deserialize(&serialized)?;
        Ok(int_item)
    }

    fn get_no_hash(&self, hash: &u64) -> Option<InternalItem<T>> {
        return if let Ok(result) = self._load_internal(&hash) {
            Some(result)
        } else {
            None
        };
    }

    fn key_to_hash(&self, key: &[u8]) -> u64 {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        let hash = hasher.finish();
        hash
    }

    pub fn iter(&self) -> Iter<'a, T, S, Ser> {
        Iter {
            storage: Self::clone(self),
            start: 0,
            end: self.len(),
        }
    }
}

/// An iterator over the contents of the append store.
pub struct Iter<'a, T, S, Ser>
where
    T: Serialize + DeserializeOwned,
    S: ReadonlyStorage,
    Ser: Serde,
{
    storage: ReadOnlyCashMap<'a, T, S, Ser>,
    start: u32,
    end: u32,
}

impl<'a, T, S, Ser> Iterator for Iter<'a, T, S, Ser>
where
    T: Serialize + DeserializeOwned,
    S: ReadonlyStorage,
    Ser: Serde,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.start >= self.end {
            return None;
        }
        let item = self.storage.get_positions(self.start, self.start);
        self.start += 1;
        return if let Ok(mut inner) = item {
            Some(inner.pop().unwrap())
        } else {
            None
        };
    }

    // This needs to be implemented correctly for `ExactSizeIterator` to work.
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = (self.end - self.start) as usize;
        (len, Some(len))
    }

    // I implement `nth` manually because it is used in the standard library whenever
    // it wants to skip over elements, but the default implementation repeatedly calls next.
    // because that is very expensive in this case, and the items are just discarded, we wan
    // do better here.
    // In practice, this enables cheap paging over the storage by calling:
    // `append_store.iter().skip(start).take(length).collect()`
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.start = self.start.saturating_add(n as u32);
        self.next()
    }
}

impl<'a, T, S, Ser> IntoIterator for ReadOnlyCashMap<'a, T, S, Ser>
where
    T: Serialize + DeserializeOwned,
    S: ReadonlyStorage,
    Ser: Serde,
{
    type Item = T;
    type IntoIter = Iter<'a, T, S, Ser>;

    fn into_iter(self) -> Iter<'a, T, S, Ser> {
        let end = self.len();
        Iter {
            storage: self,
            start: 0,
            end,
        }
    }
}

// Manual `Clone` implementation because the default one tries to clone the Storage??
impl<'a, T, S, Ser> Clone for ReadOnlyCashMap<'a, T, S, Ser>
where
    T: Serialize + DeserializeOwned,
    S: ReadonlyStorage,
    Ser: Serde,
{
    fn clone(&self) -> Self {
        Self {
            storage: &self.storage,
            item_type: self.item_type.clone(),
            serialization_type: self.serialization_type,
            prefix: self.prefix.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    use cosmwasm_std::testing::MockStorage;

    use secret_toolkit::serialization::Json;

    use super::*;

    #[derive(Serialize, Deserialize, Eq, PartialEq, Debug, Clone)]
    struct Foo {
        string: String,
        number: i32,
    }
    #[test]
    fn test_hashmap_perf_insert() -> StdResult<()> {
        let mut storage = MockStorage::new();

        let total_items = 10000;

        let mut cashmap = CashMap::attach(&mut storage);

        for i in 0..total_items {
            cashmap.insert(&(i as i32).to_be_bytes(), i)?;
        }

        Ok(())
    }

    #[test]
    fn test_hashmap_perf_insert_remove() -> StdResult<()> {
        let mut storage = MockStorage::new();

        let total_items = 10000;

        let mut cashmap = CashMap::attach(&mut storage);

        for i in 0..total_items {
            cashmap.insert(&(i as i32).to_be_bytes(), i)?;
        }

        for i in 0..total_items {
            cashmap.remove(&(i as i32).to_be_bytes())?;
        }

        assert_eq!(cashmap.len(), 0);

        Ok(())
    }

    #[test]
    fn test_hashmap_paging() -> StdResult<()> {
        let mut storage = MockStorage::new();

        let page_size = 50;
        let total_items = 10000;
        let mut cashmap = CashMap::attach(&mut storage);

        for i in 0..total_items {
            cashmap.insert(&(i as i32).to_be_bytes(), i)?;
        }

        for i in 0..((total_items / page_size) - 1) {
            let start_page = i;

            let values = cashmap.paging(start_page, page_size)?;

            for (index, value) in values.iter().enumerate() {
                assert_eq!(value, &(page_size * start_page + index as u32))
            }
        }

        Ok(())
    }

    #[test]
    fn test_hashmap_insert_multiple() -> StdResult<()> {
        let mut storage = MockStorage::new();

        let mut typed_store_mut = CashMap::attach(&mut storage);
        let foo1 = Foo {
            string: "string one".to_string(),
            number: 1111,
        };
        let foo2 = Foo {
            string: "string two".to_string(),
            number: 1111,
        };

        typed_store_mut.insert(b"key1", foo1.clone())?;
        typed_store_mut.insert(b"key2", foo2.clone())?;

        let read_foo1 = typed_store_mut.load(b"key1")?;
        let read_foo2 = typed_store_mut.load(b"key2")?;

        assert_eq!(foo1, read_foo1);
        assert_eq!(foo2, read_foo2);
        Ok(())
    }

    #[test]
    fn test_hashmap_insert_get() -> StdResult<()> {
        let mut storage = MockStorage::new();

        let mut typed_store_mut = CashMap::attach(&mut storage);
        let foo1 = Foo {
            string: "string one".to_string(),
            number: 1111,
        };

        typed_store_mut.insert(b"key1", foo1.clone())?;
        let read_foo1 = typed_store_mut.get(b"key1").unwrap();
        assert_eq!(foo1, read_foo1);

        Ok(())
    }

    #[test]
    fn test_hashmap_insert_contains() -> StdResult<()> {
        let mut storage = MockStorage::new();

        let mut typed_store_mut = CashMap::attach(&mut storage);
        let foo1 = Foo {
            string: "string one".to_string(),
            number: 1111,
        };

        typed_store_mut.insert(b"key1", foo1.clone())?;
        let contains_k1 = typed_store_mut.contains(b"key1");

        assert_eq!(contains_k1, true);

        Ok(())
    }

    #[test]
    fn test_hashmap_insert_remove() -> StdResult<()> {
        let mut storage = MockStorage::new();

        let mut typed_store_mut = CashMap::attach(&mut storage);
        let foo1 = Foo {
            string: "string one".to_string(),
            number: 1111,
        };

        typed_store_mut.insert(b"key1", foo1.clone())?;
        let before_remove_foo1 = typed_store_mut.get(b"key1");

        assert!(before_remove_foo1.is_some());
        assert_eq!(foo1, before_remove_foo1.unwrap());

        typed_store_mut.remove(b"key1")?;

        let result = typed_store_mut.get(b"key1");
        assert!(result.is_none());

        Ok(())
    }

    #[test]
    fn test_hashmap_iter() -> StdResult<()> {
        let mut storage = MockStorage::new();

        let mut hashmap = CashMap::attach(&mut storage);
        let foo1 = Foo {
            string: "string one".to_string(),
            number: 1111,
        };
        let foo2 = Foo {
            string: "string two".to_string(),
            number: 1111,
        };

        hashmap.insert(b"key1", foo1.clone())?;
        hashmap.insert(b"key2", foo2.clone())?;

        let mut x = hashmap.as_readonly().iter();
        let (len, _) = x.size_hint();
        assert_eq!(len, 2);

        assert_eq!(x.next().unwrap(), foo1);

        assert_eq!(x.next().unwrap(), foo2);

        Ok(())
    }

    #[test]
    fn test_hashmap_overwrite() -> StdResult<()> {
        let mut storage = MockStorage::new();

        let mut hashmap = CashMap::attach(&mut storage);
        let foo1 = Foo {
            string: "string one".to_string(),
            number: 1111,
        };
        let foo2 = Foo {
            string: "string two".to_string(),
            number: 2222,
        };

        hashmap.insert(b"key1", foo1.clone())?;
        hashmap.insert(b"key1", foo2.clone())?;

        let foo3 = hashmap.get(b"key1").unwrap();

        assert_eq!(foo3, foo2);

        Ok(())
    }

    #[test]
    fn test_typed_store() -> StdResult<()> {
        let mut storage = MockStorage::new();

        let mut typed_store_mut = CashMap::attach(&mut storage);
        let foo1 = Foo {
            string: "string one".to_string(),
            number: 1111,
        };
        let foo2 = Foo {
            string: "string one".to_string(),
            number: 1111,
        };
        typed_store_mut.insert(b"key1", foo1.clone())?;
        typed_store_mut.insert(b"key2", foo2.clone())?;

        let read_foo1 = typed_store_mut.get(b"key1").unwrap();
        let read_foo2 = typed_store_mut.get(b"key2").unwrap();

        assert_eq!(foo1, read_foo1);
        assert_eq!(foo2, read_foo2);

        // show that it loads foo1 before removal
        let before_remove_foo1 = typed_store_mut.get(b"key1");
        assert!(before_remove_foo1.is_some());
        assert_eq!(foo1, before_remove_foo1.unwrap());
        // and returns None after removal
        typed_store_mut.remove(b"key1")?;
        let removed_foo1 = typed_store_mut.get(b"key1");
        assert!(removed_foo1.is_none());

        // show what happens when reading from keys that have not been set yet.
        assert!(typed_store_mut.get(b"key3").is_none());

        // Try to load it with the wrong format
        let typed_store =
            ReadOnlyCashMap::<i32, _, _>::attach_with_serialization(&storage, Json, None);
        match typed_store.load(b"key2") {
            Err(StdError::ParseErr { target, msg, .. })
                if target == "secret_staking::cashmap::InternalItem<i32>"
                    && msg == "Invalid type" => {}
            other => panic!("unexpected value: {:?}", other),
        }

        Ok(())
    }
}
