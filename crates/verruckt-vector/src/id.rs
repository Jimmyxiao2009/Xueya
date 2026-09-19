use uuid::Uuid;

/// External, stable vector identifier. Internally the engine uses dense u32
/// row IDs; application chunk UUIDs map losslessly through this u128 wrapper.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct VectorId(pub u128);

impl VectorId {
    #[inline]
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid.as_u128())
    }

    #[inline]
    pub fn to_uuid(self) -> Uuid {
        Uuid::from_u128(self.0)
    }

    #[inline]
    pub fn to_le_bytes(self) -> [u8; 16] {
        self.0.to_le_bytes()
    }

    #[inline]
    pub fn from_le_bytes(bytes: [u8; 16]) -> Self {
        Self(u128::from_le_bytes(bytes))
    }

    #[inline]
    pub fn random() -> Self {
        Self(Uuid::new_v4().as_u128())
    }
}

impl From<Uuid> for VectorId {
    fn from(uuid: Uuid) -> Self {
        Self::from_uuid(uuid)
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
