//! Config structures

use std::{collections::BTreeMap, path::Path};

use serde::{Serialize, Deserialize, de::Visitor};

use crate::error::Error;

/// Config data read from Megaton.toml
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MegatonConfig {
    /// The `[module]` section
    pub module: Module,

    /// The `[lang]` section
    pub lang: Option<Lang>,

    /// The `[make]` section
    pub make: ProfileContainer<Make>,
}

impl MegatonConfig {
    /// Load a config from a file
    pub fn from_path<S>(path: S) -> Result<Self, Error> where S: AsRef<Path> {
        let path = path.as_ref();
        let config = std::fs::read_to_string(path)
            .map_err(|e| Error::ReadConfig(path.display().to_string(), e))?;
        let config = toml::from_str(&config)
            .map_err(|e| Error::ParseConfig(e.to_string()))?;
        Ok(config)
    }
}

/// Config in the `[module]` section
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Module {
    /// The name of the module, used as the target name of the final binary.
    pub name: String,
    /// The title ID as a 64-bit integer, used for generating the npdm file.
    pub title_id: u64,
}

impl Module {
    /// Get the title ID as a lower-case hex string
    pub fn title_id_hex(&self) -> String {
        format!("{:016x}", self.title_id)
    }
}

/// Config in the `[lang]` section
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Lang {
    /// Options for the clangd language server
    pub clangd: Option<LangClangd>,
}

/// Language options for clangd
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LangClangd {
    /// The path to output the `.clangd` file
    pub output: String,
}

impl Default for LangClangd {
    fn default() -> Self {
        Self {
            output: ".clangd".to_string(),
        }
    }
}

/// Config in the `[make]` section
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Make {
    /// Entry point symbol for the module
    pub entry: Option<String>,

    /// If built-in compiler flags should not be added
    ///
    /// `-I` and `-D` from includes and defines will still be added
    pub no_default_flags: Option<bool>,

    /// C/C++ Source directories, relative to Megaton.toml
    #[serde(default)]
    pub sources: Vec<String>,

    /// C/C++ Include directories, relative to Megaton.toml
    #[serde(default)]
    pub includes: Vec<String>,

    /// Extra defines
    ///
    /// These will be added to the command line as `-D<define>`
    #[serde(default)]
    pub defines: Vec<String>,

    /// Linker scripts
    #[serde(default)]
    pub ld_scripts: Vec<String>,

    /// Extra macros
    #[serde(default)]
    pub extra: Vec<KeyVal>,
}

impl Profilable for Make {
    fn extend(&mut self, other: &Self) {
        if let Some(entry) = other.entry.clone() {
            self.entry = Some(entry);
        }
        if let Some(no_default_flags) = other.no_default_flags {
            self.no_default_flags = Some(no_default_flags);
        }
        self.sources.extend(other.sources.iter().cloned());
        self.includes.extend(other.includes.iter().cloned());
        self.defines.extend(other.defines.iter().cloned());
        self.ld_scripts.extend(other.ld_scripts.iter().cloned());
        self.extra.extend(other.extra.iter().cloned());
    }
}

/// Generic config section that can be extended with profiles
///
/// For example, the `[make]` section can have profiles with `[make.profiles.<name>]`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProfileContainer<T> where T: Profilable + Clone {
    #[serde(flatten)]
    pub base: T,
    #[serde(default)]
    pub profiles: BTreeMap<String, T>,
}

impl<T> ProfileContainer<T> where T: Profilable  + Clone{
    /// Get a profile by name
    ///
    /// If the name is "none", or there is no profile with that name,
    /// the base profile will be returned. Otherwise, returns the base profile
    /// extended with the profile with the given name.
    pub fn get_profile(&self, name: &str) -> T {
        let mut base = self.base.clone();
        if name != "none" {
            if let Some(profile) = self.profiles.get(name) {
                base.extend(profile);
            }
        }
        base
    }
}

/// A trait for extending a config section with a profile
pub trait Profilable {
    /// Extend this config section with another
    fn extend(&mut self, other: &Self);
}

/// A single key-value pair converted from a map
#[derive(Debug, Clone, Default, PartialEq)]
pub struct KeyVal {
    pub key: String,
    pub val: String,
}
impl<'de> Deserialize<'de> for KeyVal {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>, {
        deserializer.deserialize_map(KeyValVisitor)
    }
}
impl Serialize for KeyVal {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer, {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry(&self.key, &self.val)?;
        map.end()
    }
}

struct KeyValVisitor;
impl<'de> Visitor<'de> for KeyValVisitor {
    type Value = KeyVal;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(formatter, "a mapping value with a single key and non-mapping value")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::MapAccess<'de>, {
        let (key, val) = match map.next_entry::<String, Val>()? {
            Some((key, val)) => {
                (key, val)
            },
            None => return Err(serde::de::Error::custom("mapping must be non-empty")),
        };

        if map.next_key::<String>()?.is_some() {
            return Err(serde::de::Error::custom("mapping must have only one key-value pair"));
        }

        Ok(KeyVal {
            key,
            val: val.0,
        })
    }
}


macro_rules! impl_visit_to_string {
    ($func:ident, $t:ty) => {
        fn $func<E>(self, x: $t) -> Result<Self::Value, E>
            where
                E: serde::de::Error, {
            Ok(Val(x.to_string()))
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
struct Val(String);
impl<'de> Deserialize<'de> for Val {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>, {
        deserializer.deserialize_any(ValVisitor)
    }
}

struct ValVisitor;
impl<'de> Visitor<'de> for ValVisitor {
    type Value = Val;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(formatter, "a non-mapping value")
    }

    impl_visit_to_string!(visit_bool, bool);

    fn visit_borrowed_bytes<E>(self, v: &'de [u8]) -> Result<Self::Value, E>
        where
            E: serde::de::Error, {
        self.visit_byte_buf(v.to_vec())
    }

    impl_visit_to_string!(visit_borrowed_str, &'de str);

    fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Self::Value, E>
        where
            E: serde::de::Error, {
        String::from_utf8(v)
            .map_err(|_| serde::de::Error::custom("value must be valid utf-8"))
            .map(Val)
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
        where
            E: serde::de::Error, {
        self.visit_byte_buf(v.to_vec())
    }

    impl_visit_to_string!(visit_char, char);

    // enum not allowed

    impl_visit_to_string!(visit_f32, f32);
    impl_visit_to_string!(visit_f64, f64);
    impl_visit_to_string!(visit_i128, i128);
    impl_visit_to_string!(visit_i16, i16);
    impl_visit_to_string!(visit_i32, i32);
    impl_visit_to_string!(visit_i64, i64);
    impl_visit_to_string!(visit_i8, i8);

    // map not allowed
    // newtype_struct not allowed

    fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error, {
        Ok(Val("".to_string()))
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::SeqAccess<'de>, {
        let mut s = Vec::new();
        while let Some(x) = seq.next_element::<Val>()? {
            s.push(x.0);
        }
        Ok(Val(s.join(" ")))
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: serde::Deserializer<'de>, {
        Deserialize::deserialize(deserializer)
    }

    impl_visit_to_string!(visit_str, &str);

    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
        where
            E: serde::de::Error, {
        Ok(Val(v))
    }

    impl_visit_to_string!(visit_u128, u128);
    impl_visit_to_string!(visit_u16, u16);
    impl_visit_to_string!(visit_u32, u32);
    impl_visit_to_string!(visit_u64, u64);
    impl_visit_to_string!(visit_u8, u8);

    fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: serde::de::Error, {
        Ok(Val("".to_string()))
    }
}
