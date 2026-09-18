use uuid::Uuid;

/// External, stable vector identifier.
///
/// Internally the engine uses dense `u32` node ids (see `idmap`); external ids
/// are what callers (the Verruckt knowledge base) store and reference. A
/// `u128` is exactly the width of a UUID, so chunk ids map losslessly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct VectorId(pub u128);

impl VectorId {
    /// Build from a UUID (the Verruckt chunk id representation).
    #[inline]
    pub fn from_uuid(uuid: Uuid) -> Self {
        VectorId(uuid.as_u128())
    }

    /// Convert back to a UUID.
    #[inline]
    pub fn to_uuid(self) -> Uuid {
        Uuid::from_u128(self.0)
    }

    /// Little-endian 16-byte representation used by the on-disk format.
    #[inline]
    pub fn to_le_bytes(self) -> [u8; 16] {
        self.0.to_le_bytes()
    }

    /// Parse a little-endian 16-byte representation.
    #[inline]
    pub fn from_le_bytes(bytes: [u8; 16]) -> Self {
        VectorId(u128::from_le_bytes(bytes))
    }

    /// A fresh random id (test / tooling convenience).
    #[inline]
    pub fn random() -> Self {
        VectorId(Uuid::new_v4().as_u128())
    }
}

impl From<Uuid> for VectorId {
    fn from(uuid: Uuid) -> Self {
        VectorId::from_uuid(uuid)
    }
}

impl From<VectorId> for Uuid {
    fn from(id: VectorId) -> Self {
        id.to_uuid()
    }
}

impl std::fmt::Display for VectorId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_uuid())
    }
}
