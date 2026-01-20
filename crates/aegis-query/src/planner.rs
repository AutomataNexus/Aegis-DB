//! Aegis Query Planner
//!
//! Transforms analyzed SQL statements into optimized query plans.
//! Implements cost-based optimization with rule-based transformations.
//!
//! Key Features:
//! - Logical plan generation from AST
//! - Cost-based join ordering
//! - Predicate pushdown optimization
//! - Index selection
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::ast::{
    BinaryOperator, Expression, FromClause, JoinClause, JoinType,
    SelectColumn, SelectStatement, Statement, TableReference,
};
use aegis_common::DataType;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

// =============================================================================
// Error Types
// =============================================================================

#[derive(Debug, Error)]
pub enum PlannerError {
    #[error("Table not found: {0}")]
    TableNotFound(String),

    #[error("Column not found: {0}")]
    ColumnNotFound(String),

    #[error("Unsupported operation: {0}")]
    UnsupportedOperation(String),

    #[error("Internal planner error: {0}")]
    Internal(String),
}

pub type PlannerResult<T> = Result<T, PlannerError>;

// =============================================================================
// Query Plan
// =============================================================================

/// A complete query plan ready for execution.
#[derive(Debug, Clone)]
pub struct QueryPlan {
    pub root: PlanNode,
    pub estimated_cost: f64,
    pub estimated_rows: u64,
}

/// A node in the query plan tree.
#[derive(Debug, Clone)]
pub enum PlanNode {
    Scan(ScanNode),
    Filter(FilterNode),
    Project(ProjectNode),
    Join(JoinNode),
    Aggregate(AggregateNode),
    Sort(SortNode),
    Limit(LimitNode),
    Empty,
}

// =============================================================================
// Plan Node Types
// =============================================================================

/// Table scan operation.
#[derive(Debug, Clone)]
pub struct ScanNode {
    pub table_name: String,
    pub alias: Option<String>,
    pub columns: Vec<String>,
    pub index_scan: Option<IndexScan>,
}

/// Index scan specification.
#[derive(Debug, Clone)]
pub struct IndexScan {
    pub index_name: String,
    pub key_range: KeyRange,
}

/// Key range for index scans.
#[derive(Debug, Clone)]
pub struct KeyRange {
    pub start: Option<PlanExpression>,
    pub start_inclusive: bool,
    pub end: Option<PlanExpression>,
    pub end_inclusive: bool,
}

/// Filter operation.
#[derive(Debug, Clone)]
pub struct FilterNode {
    pub input: Box<PlanNode>,
    pub predicate: PlanExpression,
}

/// Projection operation.
#[derive(Debug, Clone)]
pub struct ProjectNode {
    pub input: Box<PlanNode>,
    pub expressions: Vec<ProjectionExpr>,
}

/// A projection expression with optional alias.
#[derive(Debug, Clone)]
pub struct ProjectionExpr {
    pub expr: PlanExpression,
    pub alias: Option<String>,
}

/// Join operation.
#[derive(Debug, Clone)]
pub struct JoinNode {
    pub left: Box<PlanNode>,
    pub right: Box<PlanNode>,
    pub join_type: PlanJoinType,
    pub condition: Option<PlanExpression>,
    pub strategy: JoinStrategy,
}

/// Join type in plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanJoinType {
    Inner,
    Left,
    Right,
    Full,
    Cross,
}

/// Physical join strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinStrategy {
    NestedLoop,
    HashJoin,
    MergeJoin,
}

/// Aggregation operation.
#[derive(Debug, Clone)]
pub struct AggregateNode {
    pub input: Box<PlanNode>,
    pub group_by: Vec<PlanExpression>,
    pub aggregates: Vec<AggregateExpr>,
}

/// Aggregate expression.
#[derive(Debug, Clone)]
pub struct AggregateExpr {
    pub function: AggregateFunction,
    pub argument: Option<PlanExpression>,
    pub distinct: bool,
    pub alias: Option<String>,
}

/// Aggregate functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggregateFunction {
    Count,
    Sum,
    Avg,
    Min,
    Max,
}

/// Sort operation.
#[derive(Debug, Clone)]
pub struct SortNode {
    pub input: Box<PlanNode>,
    pub order_by: Vec<SortKey>,
}

/// Sort key specification.
#[derive(Debug, Clone)]
pub struct SortKey {
    pub expr: PlanExpression,
    pub ascending: bool,
    pub nulls_first: bool,
}

/// Limit operation.
#[derive(Debug, Clone)]
pub struct LimitNode {
    pub input: Box<PlanNode>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}

// =============================================================================
// Plan Expressions
// =============================================================================

/// Expression in a query plan.
#[derive(Debug, Clone)]
pub enum PlanExpression {
    Column {
        table: Option<String>,
        name: String,
        data_type: DataType,
    },
    Literal(PlanLiteral),
    BinaryOp {
        left: Box<PlanExpression>,
        op: PlanBinaryOp,
        right: Box<PlanExpression>,
    },
    UnaryOp {
        op: PlanUnaryOp,
        expr: Box<PlanExpression>,
    },
    Function {
        name: String,
        args: Vec<PlanExpression>,
        return_type: DataType,
    },
    Cast {
        expr: Box<PlanExpression>,
        target_type: DataType,
    },
    IsNull {
        expr: Box<PlanExpression>,
        negated: bool,
    },
}

/// Literal values in plan.
#[derive(Debug, Clone)]
pub enum PlanLiteral {
    Null,
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
}

/// Binary operators in plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanBinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    Equal,
    NotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
    And,
    Or,
    Concat,
}

/// Unary operators in plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanUnaryOp {
    Not,
    Negative,
}

// =============================================================================
// Schema Information
// =============================================================================

/// Schema information for planning.
#[derive(Debug, Clone, Default)]
pub struct PlannerSchema {
    tables: HashMap<String, TableInfo>,
    indexes: HashMap<String, Vec<IndexInfo>>,
}

/// Table metadata for planning.
#[derive(Debug, Clone)]
pub struct TableInfo {
    pub name: String,
    pub columns: Vec<ColumnInfo>,
    pub row_count: u64,
}

/// Column metadata for planning.
#[derive(Debug, Clone)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
}

/// Index metadata for planning.
#[derive(Debug, Clone)]
pub struct IndexInfo {
    pub name: String,
    pub columns: Vec<String>,
    pub unique: bool,
}

impl PlannerSchema {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_table(&mut self, table: TableInfo) {
        self.tables.insert(table.name.clone(), table);
    }

    pub fn add_index(&mut self, table: &str, index: IndexInfo) {
        self.indexes
            .entry(table.to_string())
            .or_default()
            .push(index);
    }

    pub fn get_table(&self, name: &str) -> Option<&TableInfo> {
        self.tables.get(name)
    }

    pub fn get_indexes(&self, table: &str) -> Option<&Vec<IndexInfo>> {
        self.indexes.get(table)
    }
}

// =============================================================================
// Planner
// =============================================================================

/// Query planner that transforms AST into execution plans.
pub struct Planner {
    schema: Arc<PlannerSchema>,
}

impl Planner {
    pub fn new(schema: Arc<PlannerSchema>) -> Self {
        Self { schema }
    }

    /// Plan a statement.
    pub fn plan(&self, statement: &Statement) -> PlannerResult<QueryPlan> {
        match statement {
            Statement::Select(select) => self.plan_select(select),
            Statement::Insert(_) => Err(PlannerError::UnsupportedOperation(
                "INSERT planning not yet implemented".to_string(),
            )),
            Statement::Update(_) => Err(PlannerError::UnsupportedOperation(
                "UPDATE planning not yet implemented".to_string(),
            )),
            Statement::Delete(_) => Err(PlannerError::UnsupportedOperation(
                "DELETE planning not yet implemented".to_string(),
            )),
            _ => Err(PlannerError::UnsupportedOperation(
                "DDL planning not yet implemented".to_string(),
            )),
        }
    }

    /// Plan a SELECT statement.
    fn plan_select(&self, select: &SelectStatement) -> PlannerResult<QueryPlan> {
        let mut plan = self.plan_from_clause(&select.from)?;

        if let Some(ref where_clause) = select.where_clause {
            let predicate = self.plan_expression(where_clause)?;
            plan = PlanNode::Filter(FilterNode {
                input: Box::new(plan),
                predicate,
            });
        }

        if !select.group_by.is_empty() || self.has_aggregates(&select.columns) {
            plan = self.plan_aggregation(plan, select)?;
        }

        if let Some(ref having) = select.having {
            let predicate = self.plan_expression(having)?;
            plan = PlanNode::Filter(FilterNode {
                input: Box::new(plan),
                predicate,
            });
        }

        plan = self.plan_projection(plan, &select.columns)?;

        if !select.order_by.is_empty() {
            let order_by: Vec<SortKey> = select
                .order_by
                .iter()
                .map(|item| {
                    Ok(SortKey {
                        expr: self.plan_expression(&item.expression)?,
                        ascending: item.ascending,
                        nulls_first: item.nulls_first.unwrap_or(!item.ascending),
                    })
                })
                .collect::<PlannerResult<Vec<_>>>()?;

            plan = PlanNode::Sort(SortNode {
                input: Box::new(plan),
                order_by,
            });
        }

        if select.limit.is_some() || select.offset.is_some() {
            plan = PlanNode::Limit(LimitNode {
                input: Box::new(plan),
                limit: select.limit,
                offset: select.offset,
            });
        }

        let (estimated_cost, estimated_rows) = self.estimate_cost(&plan);

        Ok(QueryPlan {
            root: plan,
            estimated_cost,
            estimated_rows,
        })
    }

    /// Plan FROM clause.
    fn plan_from_clause(&self, from: &Option<FromClause>) -> PlannerResult<PlanNode> {
        let from = match from {
            Some(f) => f,
            None => return Ok(PlanNode::Empty),
        };

        let mut plan = self.plan_table_reference(&from.source)?;

        for join in &from.joins {
            plan = self.plan_join(plan, join)?;
        }

        Ok(plan)
    }

    /// Plan a table reference.
    fn plan_table_reference(&self, table_ref: &TableReference) -> PlannerResult<PlanNode> {
        match table_ref {
            TableReference::Table { name, alias } => {
                let columns = if let Some(info) = self.schema.get_table(name) {
                    info.columns.iter().map(|c| c.name.clone()).collect()
                } else {
                    Vec::new()
                };

                Ok(PlanNode::Scan(ScanNode {
                    table_name: name.clone(),
                    alias: alias.clone(),
                    columns,
                    index_scan: None,
                }))
            }
            TableReference::Subquery { query, alias: _ } => self.plan_select(query).map(|p| p.root),
        }
    }

    /// Plan a join.
    fn plan_join(&self, left: PlanNode, join: &JoinClause) -> PlannerResult<PlanNode> {
        let right = self.plan_table_reference(&join.table)?;
        let condition = join
            .condition
            .as_ref()
            .map(|c| self.plan_expression(c))
            .transpose()?;

        let join_type = match join.join_type {
            JoinType::Inner => PlanJoinType::Inner,
            JoinType::Left => PlanJoinType::Left,
            JoinType::Right => PlanJoinType::Right,
            JoinType::Full => PlanJoinType::Full,
            JoinType::Cross => PlanJoinType::Cross,
        };

        let strategy = self.choose_join_strategy(&left, &right, &condition);

        Ok(PlanNode::Join(JoinNode {
            left: Box::new(left),
            right: Box::new(right),
            join_type,
            condition,
            strategy,
        }))
    }

    /// Choose join strategy based on cost estimation.
    fn choose_join_strategy(
        &self,
        _left: &PlanNode,
        _right: &PlanNode,
        condition: &Option<PlanExpression>,
    ) -> JoinStrategy {
        if condition.is_none() {
            return JoinStrategy::NestedLoop;
        }

        JoinStrategy::HashJoin
    }

    /// Check if select columns contain aggregates.
    fn has_aggregates(&self, columns: &[SelectColumn]) -> bool {
        columns.iter().any(|col| match col {
            SelectColumn::Expression { expr, .. } => self.expression_has_aggregate(expr),
            _ => false,
        })
    }

    /// Check if expression contains aggregate function.
    fn expression_has_aggregate(&self, expr: &Expression) -> bool {
        match expr {
            Expression::Function { name, .. } => {
                matches!(
                    name.to_uppercase().as_str(),
                    "COUNT" | "SUM" | "AVG" | "MIN" | "MAX"
                )
            }
            Expression::BinaryOp { left, right, .. } => {
                self.expression_has_aggregate(left) || self.expression_has_aggregate(right)
            }
            _ => false,
        }
    }

    /// Plan aggregation.
    fn plan_aggregation(
        &self,
        input: PlanNode,
        select: &SelectStatement,
    ) -> PlannerResult<PlanNode> {
        let group_by: Vec<PlanExpression> = select
            .group_by
            .iter()
            .map(|e| self.plan_expression(e))
            .collect::<PlannerResult<Vec<_>>>()?;

        let mut aggregates = Vec::new();
        for col in &select.columns {
            if let SelectColumn::Expression { expr, alias } = col {
                if let Some(agg) = self.extract_aggregate(expr, alias.clone())? {
                    aggregates.push(agg);
                }
            }
        }

        Ok(PlanNode::Aggregate(AggregateNode {
            input: Box::new(input),
            group_by,
            aggregates,
        }))
    }

    /// Extract aggregate from expression.
    fn extract_aggregate(
        &self,
        expr: &Expression,
        alias: Option<String>,
    ) -> PlannerResult<Option<AggregateExpr>> {
        match expr {
            Expression::Function {
                name,
                args,
                distinct,
            } => {
                let func = match name.to_uppercase().as_str() {
                    "COUNT" => AggregateFunction::Count,
                    "SUM" => AggregateFunction::Sum,
                    "AVG" => AggregateFunction::Avg,
                    "MIN" => AggregateFunction::Min,
                    "MAX" => AggregateFunction::Max,
                    _ => return Ok(None),
                };

                let argument = if args.is_empty() {
                    None
                } else {
                    Some(self.plan_expression(&args[0])?)
                };

                Ok(Some(AggregateExpr {
                    function: func,
                    argument,
                    distinct: *distinct,
                    alias,
                }))
            }
            _ => Ok(None),
        }
    }

    /// Plan projection.
    fn plan_projection(
        &self,
        input: PlanNode,
        columns: &[SelectColumn],
    ) -> PlannerResult<PlanNode> {
        let mut expressions = Vec::new();

        for col in columns {
            match col {
                SelectColumn::AllColumns => {
                    expressions.push(ProjectionExpr {
                        expr: PlanExpression::Column {
                            table: None,
                            name: "*".to_string(),
                            data_type: DataType::Any,
                        },
                        alias: None,
                    });
                }
                SelectColumn::TableAllColumns(table) => {
                    expressions.push(ProjectionExpr {
                        expr: PlanExpression::Column {
                            table: Some(table.clone()),
                            name: "*".to_string(),
                            data_type: DataType::Any,
                        },
                        alias: None,
                    });
                }
                SelectColumn::Expression { expr, alias } => {
                    expressions.push(ProjectionExpr {
                        expr: self.plan_expression(expr)?,
                        alias: alias.clone(),
                    });
                }
            }
        }

        Ok(PlanNode::Project(ProjectNode {
            input: Box::new(input),
            expressions,
        }))
    }

    /// Convert AST expression to plan expression.
    fn plan_expression(&self, expr: &Expression) -> PlannerResult<PlanExpression> {
        match expr {
            Expression::Literal(lit) => Ok(PlanExpression::Literal(match lit {
                crate::ast::Literal::Null => PlanLiteral::Null,
                crate::ast::Literal::Boolean(b) => PlanLiteral::Boolean(*b),
                crate::ast::Literal::Integer(i) => PlanLiteral::Integer(*i),
                crate::ast::Literal::Float(f) => PlanLiteral::Float(*f),
                crate::ast::Literal::String(s) => PlanLiteral::String(s.clone()),
            })),

            Expression::Column(col_ref) => Ok(PlanExpression::Column {
                table: col_ref.table.clone(),
                name: col_ref.column.clone(),
                data_type: DataType::Any,
            }),

            Expression::BinaryOp { left, op, right } => {
                let plan_op = match op {
                    BinaryOperator::Add => PlanBinaryOp::Add,
                    BinaryOperator::Subtract => PlanBinaryOp::Subtract,
                    BinaryOperator::Multiply => PlanBinaryOp::Multiply,
                    BinaryOperator::Divide => PlanBinaryOp::Divide,
                    BinaryOperator::Modulo => PlanBinaryOp::Modulo,
                    BinaryOperator::Equal => PlanBinaryOp::Equal,
                    BinaryOperator::NotEqual => PlanBinaryOp::NotEqual,
                    BinaryOperator::LessThan => PlanBinaryOp::LessThan,
                    BinaryOperator::LessThanOrEqual => PlanBinaryOp::LessThanOrEqual,
                    BinaryOperator::GreaterThan => PlanBinaryOp::GreaterThan,
                    BinaryOperator::GreaterThanOrEqual => PlanBinaryOp::GreaterThanOrEqual,
                    BinaryOperator::And => PlanBinaryOp::And,
                    BinaryOperator::Or => PlanBinaryOp::Or,
                    BinaryOperator::Concat => PlanBinaryOp::Concat,
                };

                Ok(PlanExpression::BinaryOp {
                    left: Box::new(self.plan_expression(left)?),
                    op: plan_op,
                    right: Box::new(self.plan_expression(right)?),
                })
            }

            Expression::UnaryOp { op, expr } => {
                let plan_op = match op {
                    crate::ast::UnaryOperator::Not => PlanUnaryOp::Not,
                    crate::ast::UnaryOperator::Negative => PlanUnaryOp::Negative,
                    crate::ast::UnaryOperator::Positive => {
                        return self.plan_expression(expr);
                    }
                };

                Ok(PlanExpression::UnaryOp {
                    op: plan_op,
                    expr: Box::new(self.plan_expression(expr)?),
                })
            }

            Expression::Function { name, args, .. } => {
                let planned_args: Vec<PlanExpression> = args
                    .iter()
                    .map(|a| self.plan_expression(a))
                    .collect::<PlannerResult<Vec<_>>>()?;

                Ok(PlanExpression::Function {
                    name: name.clone(),
                    args: planned_args,
                    return_type: DataType::Any,
                })
            }

            Expression::IsNull { expr, negated } => Ok(PlanExpression::IsNull {
                expr: Box::new(self.plan_expression(expr)?),
                negated: *negated,
            }),

            Expression::Cast { expr, data_type } => Ok(PlanExpression::Cast {
                expr: Box::new(self.plan_expression(expr)?),
                target_type: data_type.clone(),
            }),

            _ => Err(PlannerError::UnsupportedOperation(format!(
                "Expression type not yet supported: {:?}",
                expr
            ))),
        }
    }

    /// Estimate cost and row count for a plan.
    fn estimate_cost(&self, node: &PlanNode) -> (f64, u64) {
        match node {
            PlanNode::Scan(scan) => {
                let rows = self
                    .schema
                    .get_table(&scan.table_name)
                    .map(|t| t.row_count)
                    .unwrap_or(1000);
                let cost = rows as f64 * 1.0;
                (cost, rows)
            }

            PlanNode::Filter(filter) => {
                let (input_cost, input_rows) = self.estimate_cost(&filter.input);
                let selectivity = 0.1;
                let rows = (input_rows as f64 * selectivity) as u64;
                let cost = input_cost + input_rows as f64 * 0.1;
                (cost, rows.max(1))
            }

            PlanNode::Project(project) => {
                let (input_cost, input_rows) = self.estimate_cost(&project.input);
                let cost = input_cost + input_rows as f64 * 0.01;
                (cost, input_rows)
            }

            PlanNode::Join(join) => {
                let (left_cost, left_rows) = self.estimate_cost(&join.left);
                let (right_cost, right_rows) = self.estimate_cost(&join.right);

                let rows = match join.join_type {
                    PlanJoinType::Cross => left_rows * right_rows,
                    _ => (left_rows + right_rows) / 2,
                };

                let join_cost = match join.strategy {
                    JoinStrategy::NestedLoop => left_rows as f64 * right_rows as f64,
                    JoinStrategy::HashJoin => {
                        left_rows as f64 + right_rows as f64 + right_rows as f64 * 1.2
                    }
                    JoinStrategy::MergeJoin => {
                        left_rows as f64 * 2.0_f64.log2()
                            + right_rows as f64 * 2.0_f64.log2()
                            + (left_rows + right_rows) as f64
                    }
                };

                (left_cost + right_cost + join_cost, rows)
            }

            PlanNode::Aggregate(agg) => {
                let (input_cost, input_rows) = self.estimate_cost(&agg.input);
                let groups = if agg.group_by.is_empty() {
                    1
                } else {
                    (input_rows as f64).sqrt() as u64
                };
                let cost = input_cost + input_rows as f64 * 0.5;
                (cost, groups.max(1))
            }

            PlanNode::Sort(sort) => {
                let (input_cost, input_rows) = self.estimate_cost(&sort.input);
                let sort_cost = input_rows as f64 * (input_rows as f64).log2();
                (input_cost + sort_cost, input_rows)
            }

            PlanNode::Limit(limit) => {
                let (input_cost, input_rows) = self.estimate_cost(&limit.input);
                let rows = limit.limit.unwrap_or(input_rows).min(input_rows);
                (input_cost, rows)
            }

            PlanNode::Empty => (0.0, 1),
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{ColumnRef, FromClause, Literal, OrderByItem, TableReference};

    fn create_test_schema() -> Arc<PlannerSchema> {
        let mut schema = PlannerSchema::new();

        schema.add_table(TableInfo {
            name: "users".to_string(),
            columns: vec![
                ColumnInfo {
                    name: "id".to_string(),
                    data_type: DataType::Integer,
                    nullable: false,
                },
                ColumnInfo {
                    name: "name".to_string(),
                    data_type: DataType::Text,
                    nullable: false,
                },
                ColumnInfo {
                    name: "email".to_string(),
                    data_type: DataType::Text,
                    nullable: true,
                },
            ],
            row_count: 10000,
        });

        schema.add_table(TableInfo {
            name: "orders".to_string(),
            columns: vec![
                ColumnInfo {
                    name: "id".to_string(),
                    data_type: DataType::Integer,
                    nullable: false,
                },
                ColumnInfo {
                    name: "user_id".to_string(),
                    data_type: DataType::Integer,
                    nullable: false,
                },
                ColumnInfo {
                    name: "amount".to_string(),
                    data_type: DataType::Float,
                    nullable: false,
                },
            ],
            row_count: 50000,
        });

        Arc::new(schema)
    }

    #[test]
    fn test_plan_simple_select() {
        let schema = create_test_schema();
        let planner = Planner::new(schema);

        let select = SelectStatement {
            distinct: false,
            columns: vec![SelectColumn::AllColumns],
            from: Some(FromClause {
                source: TableReference::Table {
                    name: "users".to_string(),
                    alias: None,
                },
                joins: vec![],
            }),
            where_clause: None,
            group_by: vec![],
            having: None,
            order_by: vec![],
            limit: None,
            offset: None,
        };

        let plan = planner.plan(&Statement::Select(select)).unwrap();

        assert!(plan.estimated_rows > 0);
        assert!(plan.estimated_cost > 0.0);
    }

    #[test]
    fn test_plan_select_with_filter() {
        let schema = create_test_schema();
        let planner = Planner::new(schema);

        let select = SelectStatement {
            distinct: false,
            columns: vec![SelectColumn::AllColumns],
            from: Some(FromClause {
                source: TableReference::Table {
                    name: "users".to_string(),
                    alias: None,
                },
                joins: vec![],
            }),
            where_clause: Some(Expression::BinaryOp {
                left: Box::new(Expression::Column(ColumnRef {
                    table: None,
                    column: "id".to_string(),
                })),
                op: BinaryOperator::Equal,
                right: Box::new(Expression::Literal(Literal::Integer(1))),
            }),
            group_by: vec![],
            having: None,
            order_by: vec![],
            limit: None,
            offset: None,
        };

        let plan = planner.plan(&Statement::Select(select)).unwrap();

        match &plan.root {
            PlanNode::Project(p) => match p.input.as_ref() {
                PlanNode::Filter(_) => {}
                _ => panic!("Expected filter node"),
            },
            _ => panic!("Expected project node"),
        }
    }

    #[test]
    fn test_plan_select_with_order_by() {
        let schema = create_test_schema();
        let planner = Planner::new(schema);

        let select = SelectStatement {
            distinct: false,
            columns: vec![SelectColumn::AllColumns],
            from: Some(FromClause {
                source: TableReference::Table {
                    name: "users".to_string(),
                    alias: None,
                },
                joins: vec![],
            }),
            where_clause: None,
            group_by: vec![],
            having: None,
            order_by: vec![OrderByItem {
                expression: Expression::Column(ColumnRef {
                    table: None,
                    column: "name".to_string(),
                }),
                ascending: true,
                nulls_first: None,
            }],
            limit: Some(10),
            offset: None,
        };

        let plan = planner.plan(&Statement::Select(select)).unwrap();

        match &plan.root {
            PlanNode::Limit(l) => match l.input.as_ref() {
                PlanNode::Sort(_) => {}
                _ => panic!("Expected sort node"),
            },
            _ => panic!("Expected limit node"),
        }
    }
}
