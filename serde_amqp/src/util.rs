#[derive(Debug)]
pub(crate) enum NewType {
    None,
    Array,
    Dec32,
    Dec64,
    Dec128,
    Symbol,
    SymbolRef,
    Timestamp,
    Uuid,
}

impl Default for NewType {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Debug, Clone)]
pub enum IsArrayElement {
    False,
    FirstElement,
    OtherElement,
}

#[derive(Debug, Clone)]
pub enum EnumType {
    None,
    Array,
    Descriptor,
    Value,
}

impl Default for EnumType {
    fn default() -> Self {
        Self::None
    }
}

/// Described type has the descriptor as the first field
#[derive(Debug)]
pub(crate) enum FieldRole {
    Descriptor,
    Fields,
}

#[derive(Debug, Clone)]
#[repr(u8)]
pub enum StructEncoding {
    None,
    DescribedList,
    DescribedMap,
    DescribedBasic,
}

impl Default for StructEncoding {
    fn default() -> Self {
        Self::None
    }
}

pub trait TryFromSerializable<T: serde::ser::Serialize>: Sized {
    type Error: std::error::Error;

    fn try_from(value: T) -> Result<Self, Self::Error>;
}
