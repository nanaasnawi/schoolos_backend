use sqlx::{Postgres, QueryBuilder};

/// A trait to implement the Specification Pattern for dynamic SQL querying with SQLx.
pub trait Specification<'a> {
    /// Applies filters (WHERE clauses) to the QueryBuilder.
    /// Assumes `WHERE` or `AND` is already present appropriately if there are preceding conditions.
    fn apply_where(&'a self, builder: &mut QueryBuilder<'a, Postgres>);

    /// Applies necessary JOINs if the specification requires filtering on relations.
    /// By default, it does nothing.
    fn apply_joins(&'a self, _builder: &mut QueryBuilder<'a, Postgres>) {}
}

pub struct AndSpecification<'a, L: Specification<'a>, R: Specification<'a>> {
    pub left: L,
    pub right: R,
    pub _marker: std::marker::PhantomData<&'a ()>,
}

impl<'a, L: Specification<'a>, R: Specification<'a>> Specification<'a>
    for AndSpecification<'a, L, R>
{
    fn apply_where(&'a self, builder: &mut QueryBuilder<'a, Postgres>) {
        builder.push("(");
        self.left.apply_where(builder);
        builder.push(" AND ");
        self.right.apply_where(builder);
        builder.push(")");
    }

    fn apply_joins(&'a self, builder: &mut QueryBuilder<'a, Postgres>) {
        self.left.apply_joins(builder);
        self.right.apply_joins(builder);
    }
}

pub struct OrSpecification<'a, L: Specification<'a>, R: Specification<'a>> {
    pub left: L,
    pub right: R,
    pub _marker: std::marker::PhantomData<&'a ()>,
}

impl<'a, L: Specification<'a>, R: Specification<'a>> Specification<'a>
    for OrSpecification<'a, L, R>
{
    fn apply_where(&'a self, builder: &mut QueryBuilder<'a, Postgres>) {
        builder.push("(");
        self.left.apply_where(builder);
        builder.push(" OR ");
        self.right.apply_where(builder);
        builder.push(")");
    }

    fn apply_joins(&'a self, builder: &mut QueryBuilder<'a, Postgres>) {
        self.left.apply_joins(builder);
        self.right.apply_joins(builder);
    }
}

pub struct NotSpecification<'a, S: Specification<'a>> {
    pub spec: S,
    pub _marker: std::marker::PhantomData<&'a ()>,
}

impl<'a, S: Specification<'a>> Specification<'a> for NotSpecification<'a, S> {
    fn apply_where(&'a self, builder: &mut QueryBuilder<'a, Postgres>) {
        builder.push("NOT (");
        self.spec.apply_where(builder);
        builder.push(")");
    }

    fn apply_joins(&'a self, builder: &mut QueryBuilder<'a, Postgres>) {
        self.spec.apply_joins(builder);
    }
}
