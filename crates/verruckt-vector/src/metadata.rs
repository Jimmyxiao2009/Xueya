use crate::error::{Result, VectorError};
use crate::id::VectorId;

/// Lightweight per-vector metadata.
///
/// This is deliberately *not* a JSON document store: it carries exactly the
/// fields Verruckt's retriever filters on, in a fixed, compact, typed layout.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Metadata {
    pub route_id: Option<VectorId>,
    pub source_id: Option<VectorId>,
    pub knowledge_point_id: Option<VectorId>,
    /// Free-form small tag, e.g. `"chunk"`, `"summary"`. ≤ 64 bytes.
    pub kind: Option<String>,
}

/// Upper bound for `kind` strings kept in metadata (keeps rows compact).
pub const MAX_KIND_LEN: usize = 64;

impl Metadata {
    /// Metadata with no fields set.
    pub fn empty() -> Self {
        Metadata::default()
    }

    pub fn with_kind(kind: impl Into<String>) -> Self {
        Metadata {
            kind: Some(kind.into()),
            ..Metadata::default()
        }
    }

    pub fn validate(&self) -> Result<()> {
        if let Some(kind) = &self.kind {
            if kind.len() > MAX_KIND_LEN {
                return Err(VectorError::invalid_metadata(format!(
                    "kind must be <= {MAX_KIND_LEN} bytes, got {}",
                    kind.len()
                )));
            }
        }
        Ok(())
    }
}

/// Filterable metadata field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MetadataField {
    RouteId,
    SourceId,
    KnowledgePointId,
    Kind,
}

/// A single filter predicate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Predicate {
    /// `field == value`
    Eq(MetadataField, FilterValue),
    /// `field in [values]` (empty set matches nothing)
    In(MetadataField, Vec<FilterValue>),
}

/// Value a predicate compares against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterValue {
    Id(VectorId),
    Kind(String),
}

impl Predicate {
    /// Does a metadata row satisfy this predicate?
    pub fn matches(&self, meta: &Metadata) -> bool {
        match self {
            Predicate::Eq(field, value) => field_matches(field, value, meta),
            Predicate::In(field, values) => {
                values.iter().any(|value| field_matches(field, value, meta))
            }
        }
    }
}

fn field_matches(field: &MetadataField, value: &FilterValue, meta: &Metadata) -> bool {
    match (field, value) {
        (MetadataField::RouteId, FilterValue::Id(id)) => meta.route_id == Some(*id),
        (MetadataField::SourceId, FilterValue::Id(id)) => meta.source_id == Some(*id),
        (MetadataField::KnowledgePointId, FilterValue::Id(id)) => {
            meta.knowledge_point_id == Some(*id)
        }
        (MetadataField::Kind, FilterValue::Kind(kind)) => meta.kind.as_deref() == Some(kind.as_str()),
        _ => false,
    }
}

/// Metadata filter: a conjunction of predicates.
///
/// ```
/// use verruckt_vector::{MetadataFilter, MetadataField, FilterValue, VectorId};
///
/// let route = VectorId(7);
/// let sources = vec![VectorId(1), VectorId(2)];
/// let filter = MetadataFilter::new()
///     .eq(MetadataField::RouteId, FilterValue::Id(route))
///     .in_set(MetadataField::SourceId, sources.into_iter().map(FilterValue::Id));
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MetadataFilter {
    predicates: Vec<Predicate>,
}

impl MetadataFilter {
    /// Empty filter — matches everything.
    pub fn new() -> Self {
        MetadataFilter::default()
    }

    /// Add a `field == value` conjunct.
    pub fn eq(mut self, field: MetadataField, value: FilterValue) -> Self {
        self.predicates.push(Predicate::Eq(field, value));
        self
    }

    /// Add a `field in values` conjunct.
    pub fn in_set<I: IntoIterator<Item = FilterValue>>(
        mut self,
        field: MetadataField,
        values: I,
    ) -> Self {
        self.predicates
            .push(Predicate::In(field, values.into_iter().collect()));
        self
    }

    pub fn route_id_eq(self, id: VectorId) -> Self {
        self.eq(MetadataField::RouteId, FilterValue::Id(id))
    }

    pub fn route_id_in<I: IntoIterator<Item = VectorId>>(self, ids: I) -> Self {
        self.in_set(MetadataField::RouteId, ids.into_iter().map(FilterValue::Id))
    }

    pub fn source_id_eq(self, id: VectorId) -> Self {
        self.eq(MetadataField::SourceId, FilterValue::Id(id))
    }

    pub fn source_id_in<I: IntoIterator<Item = VectorId>>(self, ids: I) -> Self {
        self.in_set(MetadataField::SourceId, ids.into_iter().map(FilterValue::Id))
    }

    pub fn knowledge_point_id_eq(self, id: VectorId) -> Self {
        self.eq(MetadataField::KnowledgePointId, FilterValue::Id(id))
    }

    pub fn knowledge_point_id_in<I: IntoIterator<Item = VectorId>>(self, ids: I) -> Self {
        self.in_set(
            MetadataField::KnowledgePointId,
            ids.into_iter().map(FilterValue::Id),
        )
    }

    pub fn kind_eq(self, kind: impl Into<String>) -> Self {
        self.eq(MetadataField::Kind, FilterValue::Kind(kind.into()))
    }

    pub fn kind_in<I: IntoIterator<Item = String>>(self, kinds: I) -> Self {
        self.in_set(MetadataField::Kind, kinds.into_iter().map(FilterValue::Kind))
    }

    /// Number of conjuncts.
    pub fn len(&self) -> usize {
        self.predicates.len()
    }

    pub fn is_empty(&self) -> bool {
        self.predicates.is_empty()
    }

    /// Conjunction check.
    pub fn matches(&self, meta: &Metadata) -> bool {
        self.predicates.iter().all(|predicate| predicate.matches(meta))
    }

    #[allow(dead_code)] // kept for adapter use
    pub(crate) fn predicates(&self) -> &[Predicate] {
        &self.predicates
    }
}

impl From<Predicate> for MetadataFilter {
    fn from(predicate: Predicate) -> Self {
        MetadataFilter {
            predicates: vec![predicate],
        }
    }
}
