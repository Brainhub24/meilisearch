use std::collections::hash_map::Entry;
use std::collections::{HashSet, HashMap};
use std::path::Path;
use std::sync::RwLock;
use meilidb_schema::Schema;

mod error;
mod index;
mod update;

pub use self::error::Error;
pub use self::index::{Index, CustomSettingsIndex};

pub use self::update::DocumentsAddition;
pub use self::update::DocumentsDeletion;
pub use self::update::SynonymsAddition;
pub use self::update::SynonymsDeletion;

use self::update::apply_documents_addition;
use self::update::apply_documents_deletion;
use self::update::apply_synonyms_addition;
use self::update::apply_synonyms_deletion;

const INDEXES_KEY: &str = "indexes";

fn load_indexes(tree: &sled::Tree) -> Result<HashSet<String>, Error> {
    match tree.get(INDEXES_KEY)? {
        Some(bytes) => Ok(bincode::deserialize(&bytes)?),
        None => Ok(HashSet::new())
    }
}

pub struct Database {
    cache: RwLock<HashMap<String, Index>>,
    inner: sled::Db,
}

impl Database {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Database, Error> {
        let cache = RwLock::new(HashMap::new());
        let inner = sled::Db::open(path)?;

        let indexes = load_indexes(&inner)?;
        let database = Database { cache, inner };

        for index in indexes {
            database.open_index(&index)?;
        }

        Ok(database)
    }

    pub fn indexes(&self) -> Result<HashSet<String>, Error> {
        load_indexes(&self.inner)
    }

    fn set_indexes(&self, value: &HashSet<String>) -> Result<(), Error> {
        let bytes = bincode::serialize(value)?;
        self.inner.insert(INDEXES_KEY, bytes)?;
        Ok(())
    }

    pub fn open_index(&self, name: &str) -> Result<Option<Index>, Error> {
        {
            let cache = self.cache.read().unwrap();
            if let Some(index) = cache.get(name).cloned() {
                return Ok(Some(index))
            }
        }

        let mut cache = self.cache.write().unwrap();
        let index = match cache.entry(name.to_string()) {
            Entry::Occupied(occupied) => {
                occupied.get().clone()
            },
            Entry::Vacant(vacant) => {
                if !self.indexes()?.contains(name) {
                    return Ok(None)
                }

                let index = Index::new(self.inner.clone(), name)?;
                vacant.insert(index).clone()
            },
        };

        Ok(Some(index))
    }

    pub fn create_index(&self, name: &str, schema: Schema) -> Result<Index, Error> {
        let mut cache = self.cache.write().unwrap();

        let index = match cache.entry(name.to_string()) {
            Entry::Occupied(occupied) => {
                occupied.get().clone()
            },
            Entry::Vacant(vacant) => {
                let index = Index::with_schema(self.inner.clone(), name, schema)?;

                let mut indexes = self.indexes()?;
                indexes.insert(name.to_string());
                self.set_indexes(&indexes)?;

                vacant.insert(index).clone()
            },
        };

        Ok(index)
    }
}
