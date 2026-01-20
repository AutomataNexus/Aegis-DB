//! Aegis Query Executor
//!
//! Executes query plans using a Volcano-style iterator model.
//! Implements vectorized execution for improved performance.
//!
//! Key Features:
//! - Pull-based iterator execution
//! - Vectorized batch processing
//! - Memory-efficient result streaming
//! - Support for parallel execution
//!
//! @version 0.1.0
//! @author AutomataNexus Development Team

use crate::planner::{
    AggregateFunction, AggregateNode, FilterNode, JoinNode, JoinStrategy, LimitNode,
    PlanBinaryOp, PlanExpression, PlanJoinType, PlanLiteral, PlanNode, PlanUnaryOp,
    ProjectNode, QueryPlan, ScanNode, SortNode,
};
use aegis_common::{DataType, Row, Value};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

// =============================================================================
// Error Types
// =============================================================================

#[derive(Debug, Error)]
pub enum ExecutorError {
    #[error("Table not found: {0}")]
    TableNotFound(String),

    #[error("Column not found: {0}")]
    ColumnNotFound(String),

    #[error("Type mismatch: expected {expected}, got {actual}")]
    TypeMismatch { expected: String, actual: String },

    #[error("Division by zero")]
    DivisionByZero,

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("Execution error: {0}")]
    Internal(String),
}

pub type ExecutorResult<T> = Result<T, ExecutorError>;

// =============================================================================
// Execution Context
// =============================================================================

/// Context for query execution.
pub struct ExecutionContext {
    tables: HashMap<String, Arc<TableData>>,
    batch_size: usize,
}

/// In-memory table data for execution.
#[derive(Debug, Clone)]
pub struct TableData {
    pub name: String,
    pub columns: Vec<String>,
    pub rows: Vec<Row>,
}

impl ExecutionContext {
    pub fn new() -> Self {
        Self {
            tables: HashMap::new(),
            batch_size: 1024,
        }
    }

    pub fn with_batch_size(mut self, size: usize) -> Self {
        self.batch_size = size;
        self
    }

    pub fn add_table(&mut self, table: TableData) {
        self.tables.insert(table.name.clone(), Arc::new(table));
    }

    pub fn get_table(&self, name: &str) -> Option<Arc<TableData>> {
        self.tables.get(name).cloned()
    }

    pub fn batch_size(&self) -> usize {
        self.batch_size
    }
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Result Batch
// =============================================================================

/// A batch of result rows.
#[derive(Debug, Clone)]
pub struct ResultBatch {
    pub columns: Vec<String>,
    pub rows: Vec<Row>,
}

impl ResultBatch {
    pub fn new(columns: Vec<String>) -> Self {
        Self {
            columns,
            rows: Vec::new(),
        }
    }

    pub fn with_rows(columns: Vec<String>, rows: Vec<Row>) -> Self {
        Self { columns, rows }
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }
}

// =============================================================================
// Query Result
// =============================================================================

/// Complete query result.
#[derive(Debug, Clone)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Row>,
    pub rows_affected: u64,
}

impl QueryResult {
    pub fn new(columns: Vec<String>, rows: Vec<Row>) -> Self {
        let rows_affected = rows.len() as u64;
        Self {
            columns,
            rows,
            rows_affected,
        }
    }

    pub fn empty() -> Self {
        Self {
            columns: Vec::new(),
            rows: Vec::new(),
            rows_affected: 0,
        }
    }
}

// =============================================================================
// Executor
// =============================================================================

/// Query executor.
pub struct Executor {
    context: ExecutionContext,
}

impl Executor {
    pub fn new(context: ExecutionContext) -> Self {
        Self { context }
    }

    /// Execute a query plan.
    pub fn execute(&self, plan: &QueryPlan) -> ExecutorResult<QueryResult> {
        let mut operator = self.create_operator(&plan.root)?;
        let mut all_rows = Vec::new();
        let mut columns = Vec::new();

        while let Some(batch) = operator.next_batch()? {
            if columns.is_empty() {
                columns = batch.columns.clone();
            }
            all_rows.extend(batch.rows);
        }

        Ok(QueryResult::new(columns, all_rows))
    }

    /// Create an operator tree from a plan node.
    fn create_operator(&self, node: &PlanNode) -> ExecutorResult<Box<dyn Operator>> {
        match node {
            PlanNode::Scan(scan) => Ok(Box::new(ScanOperator::new(scan.clone(), &self.context)?)),

            PlanNode::Filter(filter) => {
                let input = self.create_operator(&filter.input)?;
                Ok(Box::new(FilterOperator::new(
                    input,
                    filter.predicate.clone(),
                )))
            }

            PlanNode::Project(project) => {
                let input = self.create_operator(&project.input)?;
                Ok(Box::new(ProjectOperator::new(
                    input,
                    project.expressions.clone(),
                )))
            }

            PlanNode::Join(join) => {
                let left = self.create_operator(&join.left)?;
                let right = self.create_operator(&join.right)?;
                Ok(Box::new(JoinOperator::new(
                    left,
                    right,
                    join.join_type,
                    join.condition.clone(),
                    join.strategy,
                )?))
            }

            PlanNode::Aggregate(agg) => {
                let input = self.create_operator(&agg.input)?;
                Ok(Box::new(AggregateOperator::new(
                    input,
                    agg.group_by.clone(),
                    agg.aggregates.clone(),
                )))
            }

            PlanNode::Sort(sort) => {
                let input = self.create_operator(&sort.input)?;
                Ok(Box::new(SortOperator::new(input, sort.order_by.clone())))
            }

            PlanNode::Limit(limit) => {
                let input = self.create_operator(&limit.input)?;
                Ok(Box::new(LimitOperator::new(input, limit.limit, limit.offset)))
            }

            PlanNode::Empty => Ok(Box::new(EmptyOperator::new())),
        }
    }
}

// =============================================================================
// Operator Trait
// =============================================================================

/// Operator in the execution pipeline.
pub trait Operator: Send {
    /// Get the next batch of results.
    fn next_batch(&mut self) -> ExecutorResult<Option<ResultBatch>>;

    /// Get output column names.
    fn columns(&self) -> &[String];
}

// =============================================================================
// Scan Operator
// =============================================================================

struct ScanOperator {
    table: Arc<TableData>,
    columns: Vec<String>,
    position: usize,
    batch_size: usize,
}

impl ScanOperator {
    fn new(scan: ScanNode, context: &ExecutionContext) -> ExecutorResult<Self> {
        let table = context
            .get_table(&scan.table_name)
            .ok_or_else(|| ExecutorError::TableNotFound(scan.table_name.clone()))?;

        let columns = if scan.columns.is_empty() {
            table.columns.clone()
        } else {
            scan.columns.clone()
        };

        Ok(Self {
            table,
            columns,
            position: 0,
            batch_size: context.batch_size(),
        })
    }
}

impl Operator for ScanOperator {
    fn next_batch(&mut self) -> ExecutorResult<Option<ResultBatch>> {
        if self.position >= self.table.rows.len() {
            return Ok(None);
        }

        let end = (self.position + self.batch_size).min(self.table.rows.len());
        let rows: Vec<Row> = self.table.rows[self.position..end].to_vec();
        self.position = end;

        Ok(Some(ResultBatch::with_rows(self.columns.clone(), rows)))
    }

    fn columns(&self) -> &[String] {
        &self.columns
    }
}

// =============================================================================
// Filter Operator
// =============================================================================

struct FilterOperator {
    input: Box<dyn Operator>,
    predicate: PlanExpression,
    columns: Vec<String>,
}

impl FilterOperator {
    fn new(input: Box<dyn Operator>, predicate: PlanExpression) -> Self {
        let columns = input.columns().to_vec();
        Self {
            input,
            predicate,
            columns,
        }
    }

    fn evaluate_predicate(&self, row: &Row, columns: &[String]) -> ExecutorResult<bool> {
        let value = evaluate_expression(&self.predicate, row, columns)?;
        match value {
            Value::Boolean(b) => Ok(b),
            Value::Null => Ok(false),
            _ => Err(ExecutorError::TypeMismatch {
                expected: "boolean".to_string(),
                actual: format!("{:?}", value),
            }),
        }
    }
}

impl Operator for FilterOperator {
    fn next_batch(&mut self) -> ExecutorResult<Option<ResultBatch>> {
        while let Some(batch) = self.input.next_batch()? {
            let filtered: Vec<Row> = batch
                .rows
                .into_iter()
                .filter(|row| self.evaluate_predicate(row, &batch.columns).unwrap_or(false))
                .collect();

            if !filtered.is_empty() {
                return Ok(Some(ResultBatch::with_rows(self.columns.clone(), filtered)));
            }
        }
        Ok(None)
    }

    fn columns(&self) -> &[String] {
        &self.columns
    }
}

// =============================================================================
// Project Operator
// =============================================================================

struct ProjectOperator {
    input: Box<dyn Operator>,
    expressions: Vec<crate::planner::ProjectionExpr>,
    columns: Vec<String>,
    input_columns: Vec<String>,
}

impl ProjectOperator {
    fn new(input: Box<dyn Operator>, expressions: Vec<crate::planner::ProjectionExpr>) -> Self {
        let input_columns = input.columns().to_vec();

        let columns: Vec<String> = expressions
            .iter()
            .enumerate()
            .map(|(i, expr)| {
                expr.alias
                    .clone()
                    .unwrap_or_else(|| format!("column_{}", i))
            })
            .collect();

        Self {
            input,
            expressions,
            columns,
            input_columns,
        }
    }
}

impl Operator for ProjectOperator {
    fn next_batch(&mut self) -> ExecutorResult<Option<ResultBatch>> {
        if let Some(batch) = self.input.next_batch()? {
            let mut result_rows = Vec::with_capacity(batch.rows.len());

            for row in &batch.rows {
                let mut projected_values = Vec::with_capacity(self.expressions.len());

                for expr in &self.expressions {
                    let value = evaluate_expression(&expr.expr, row, &self.input_columns)?;
                    projected_values.push(value);
                }

                result_rows.push(Row {
                    values: projected_values,
                });
            }

            Ok(Some(ResultBatch::with_rows(
                self.columns.clone(),
                result_rows,
            )))
        } else {
            Ok(None)
        }
    }

    fn columns(&self) -> &[String] {
        &self.columns
    }
}

// =============================================================================
// Join Operator
// =============================================================================

struct JoinOperator {
    left: Box<dyn Operator>,
    right: Box<dyn Operator>,
    join_type: PlanJoinType,
    condition: Option<PlanExpression>,
    _strategy: JoinStrategy,
    columns: Vec<String>,
    left_columns: Vec<String>,
    right_columns: Vec<String>,
    right_data: Option<Vec<Row>>,
    left_batch: Option<ResultBatch>,
    left_row_idx: usize,
    right_row_idx: usize,
}

impl JoinOperator {
    fn new(
        left: Box<dyn Operator>,
        right: Box<dyn Operator>,
        join_type: PlanJoinType,
        condition: Option<PlanExpression>,
        strategy: JoinStrategy,
    ) -> ExecutorResult<Self> {
        let left_columns = left.columns().to_vec();
        let right_columns = right.columns().to_vec();

        let mut columns = left_columns.clone();
        columns.extend(right_columns.clone());

        Ok(Self {
            left,
            right,
            join_type,
            condition,
            _strategy: strategy,
            columns,
            left_columns,
            right_columns,
            right_data: None,
            left_batch: None,
            left_row_idx: 0,
            right_row_idx: 0,
        })
    }

    fn materialize_right(&mut self) -> ExecutorResult<()> {
        if self.right_data.is_some() {
            return Ok(());
        }

        let mut all_rows = Vec::new();
        while let Some(batch) = self.right.next_batch()? {
            all_rows.extend(batch.rows);
        }
        self.right_data = Some(all_rows);
        Ok(())
    }

    fn evaluate_join_condition(&self, left: &Row, right: &Row) -> ExecutorResult<bool> {
        match &self.condition {
            None => Ok(true),
            Some(expr) => {
                let mut combined = left.values.clone();
                combined.extend(right.values.clone());
                let combined_row = Row { values: combined };

                let value = evaluate_expression(expr, &combined_row, &self.columns)?;
                match value {
                    Value::Boolean(b) => Ok(b),
                    Value::Null => Ok(false),
                    _ => Ok(false),
                }
            }
        }
    }
}

impl Operator for JoinOperator {
    fn next_batch(&mut self) -> ExecutorResult<Option<ResultBatch>> {
        self.materialize_right()?;
        let right_data = self.right_data.as_ref().unwrap();

        let mut result_rows = Vec::new();

        loop {
            if self.left_batch.is_none() {
                self.left_batch = self.left.next_batch()?;
                self.left_row_idx = 0;
                self.right_row_idx = 0;

                if self.left_batch.is_none() {
                    break;
                }
            }

            let left_batch = self.left_batch.as_ref().unwrap();

            while self.left_row_idx < left_batch.rows.len() {
                let left_row = &left_batch.rows[self.left_row_idx];

                while self.right_row_idx < right_data.len() {
                    let right_row = &right_data[self.right_row_idx];
                    self.right_row_idx += 1;

                    if self.evaluate_join_condition(left_row, right_row)? {
                        let mut combined = left_row.values.clone();
                        combined.extend(right_row.values.clone());
                        result_rows.push(Row { values: combined });

                        if result_rows.len() >= 1024 {
                            return Ok(Some(ResultBatch::with_rows(
                                self.columns.clone(),
                                result_rows,
                            )));
                        }
                    }
                }

                self.left_row_idx += 1;
                self.right_row_idx = 0;
            }

            self.left_batch = None;
        }

        if result_rows.is_empty() {
            Ok(None)
        } else {
            Ok(Some(ResultBatch::with_rows(
                self.columns.clone(),
                result_rows,
            )))
        }
    }

    fn columns(&self) -> &[String] {
        &self.columns
    }
}

// =============================================================================
// Aggregate Operator
// =============================================================================

struct AggregateOperator {
    input: Box<dyn Operator>,
    _group_by: Vec<PlanExpression>,
    aggregates: Vec<crate::planner::AggregateExpr>,
    columns: Vec<String>,
    input_columns: Vec<String>,
    done: bool,
}

impl AggregateOperator {
    fn new(
        input: Box<dyn Operator>,
        group_by: Vec<PlanExpression>,
        aggregates: Vec<crate::planner::AggregateExpr>,
    ) -> Self {
        let input_columns = input.columns().to_vec();

        let columns: Vec<String> = aggregates
            .iter()
            .enumerate()
            .map(|(i, agg)| {
                agg.alias
                    .clone()
                    .unwrap_or_else(|| format!("agg_{}", i))
            })
            .collect();

        Self {
            input,
            _group_by: group_by,
            aggregates,
            columns,
            input_columns,
            done: false,
        }
    }
}

impl Operator for AggregateOperator {
    fn next_batch(&mut self) -> ExecutorResult<Option<ResultBatch>> {
        if self.done {
            return Ok(None);
        }

        let mut accumulators: Vec<Accumulator> = self
            .aggregates
            .iter()
            .map(|agg| Accumulator::new(agg.function))
            .collect();

        while let Some(batch) = self.input.next_batch()? {
            for row in &batch.rows {
                for (i, agg) in self.aggregates.iter().enumerate() {
                    let value = if let Some(ref arg) = agg.argument {
                        evaluate_expression(arg, row, &self.input_columns)?
                    } else {
                        Value::Integer(1)
                    };
                    accumulators[i].accumulate(&value)?;
                }
            }
        }

        let result_values: Vec<Value> = accumulators.iter().map(|acc| acc.finalize()).collect();

        self.done = true;

        Ok(Some(ResultBatch::with_rows(
            self.columns.clone(),
            vec![Row {
                values: result_values,
            }],
        )))
    }

    fn columns(&self) -> &[String] {
        &self.columns
    }
}

/// Accumulator for aggregate functions.
struct Accumulator {
    function: AggregateFunction,
    count: i64,
    sum: f64,
    min: Option<Value>,
    max: Option<Value>,
}

impl Accumulator {
    fn new(function: AggregateFunction) -> Self {
        Self {
            function,
            count: 0,
            sum: 0.0,
            min: None,
            max: None,
        }
    }

    fn accumulate(&mut self, value: &Value) -> ExecutorResult<()> {
        if matches!(value, Value::Null) {
            return Ok(());
        }

        self.count += 1;

        match self.function {
            AggregateFunction::Count => {}
            AggregateFunction::Sum | AggregateFunction::Avg => {
                self.sum += value_to_f64(value)?;
            }
            AggregateFunction::Min => {
                if self.min.is_none() || compare_values(value, self.min.as_ref().unwrap())? == std::cmp::Ordering::Less {
                    self.min = Some(value.clone());
                }
            }
            AggregateFunction::Max => {
                if self.max.is_none() || compare_values(value, self.max.as_ref().unwrap())? == std::cmp::Ordering::Greater {
                    self.max = Some(value.clone());
                }
            }
        }

        Ok(())
    }

    fn finalize(&self) -> Value {
        match self.function {
            AggregateFunction::Count => Value::Integer(self.count),
            AggregateFunction::Sum => Value::Float(self.sum),
            AggregateFunction::Avg => {
                if self.count == 0 {
                    Value::Null
                } else {
                    Value::Float(self.sum / self.count as f64)
                }
            }
            AggregateFunction::Min => self.min.clone().unwrap_or(Value::Null),
            AggregateFunction::Max => self.max.clone().unwrap_or(Value::Null),
        }
    }
}

// =============================================================================
// Sort Operator
// =============================================================================

struct SortOperator {
    input: Box<dyn Operator>,
    order_by: Vec<crate::planner::SortKey>,
    columns: Vec<String>,
    sorted_data: Option<Vec<Row>>,
    position: usize,
}

impl SortOperator {
    fn new(input: Box<dyn Operator>, order_by: Vec<crate::planner::SortKey>) -> Self {
        let columns = input.columns().to_vec();
        Self {
            input,
            order_by,
            columns,
            sorted_data: None,
            position: 0,
        }
    }
}

impl Operator for SortOperator {
    fn next_batch(&mut self) -> ExecutorResult<Option<ResultBatch>> {
        if self.sorted_data.is_none() {
            let mut all_rows = Vec::new();
            while let Some(batch) = self.input.next_batch()? {
                all_rows.extend(batch.rows);
            }

            let columns = self.columns.clone();
            let order_by = self.order_by.clone();

            all_rows.sort_by(|a, b| {
                for key in &order_by {
                    let a_val = evaluate_expression(&key.expr, a, &columns).unwrap_or(Value::Null);
                    let b_val = evaluate_expression(&key.expr, b, &columns).unwrap_or(Value::Null);

                    let cmp = compare_values(&a_val, &b_val).unwrap_or(std::cmp::Ordering::Equal);

                    if cmp != std::cmp::Ordering::Equal {
                        return if key.ascending {
                            cmp
                        } else {
                            cmp.reverse()
                        };
                    }
                }
                std::cmp::Ordering::Equal
            });

            self.sorted_data = Some(all_rows);
        }

        let data = self.sorted_data.as_ref().unwrap();

        if self.position >= data.len() {
            return Ok(None);
        }

        let end = (self.position + 1024).min(data.len());
        let rows = data[self.position..end].to_vec();
        self.position = end;

        Ok(Some(ResultBatch::with_rows(self.columns.clone(), rows)))
    }

    fn columns(&self) -> &[String] {
        &self.columns
    }
}

// =============================================================================
// Limit Operator
// =============================================================================

struct LimitOperator {
    input: Box<dyn Operator>,
    limit: Option<u64>,
    offset: Option<u64>,
    columns: Vec<String>,
    rows_skipped: u64,
    rows_returned: u64,
}

impl LimitOperator {
    fn new(input: Box<dyn Operator>, limit: Option<u64>, offset: Option<u64>) -> Self {
        let columns = input.columns().to_vec();
        Self {
            input,
            limit,
            offset,
            columns,
            rows_skipped: 0,
            rows_returned: 0,
        }
    }
}

impl Operator for LimitOperator {
    fn next_batch(&mut self) -> ExecutorResult<Option<ResultBatch>> {
        if let Some(limit) = self.limit {
            if self.rows_returned >= limit {
                return Ok(None);
            }
        }

        while let Some(batch) = self.input.next_batch()? {
            let mut rows = batch.rows;

            let offset = self.offset.unwrap_or(0);
            if self.rows_skipped < offset {
                let skip = (offset - self.rows_skipped) as usize;
                if skip >= rows.len() {
                    self.rows_skipped += rows.len() as u64;
                    continue;
                }
                rows = rows[skip..].to_vec();
                self.rows_skipped = offset;
            }

            if let Some(limit) = self.limit {
                let remaining = limit - self.rows_returned;
                if rows.len() as u64 > remaining {
                    rows.truncate(remaining as usize);
                }
            }

            if rows.is_empty() {
                continue;
            }

            self.rows_returned += rows.len() as u64;

            return Ok(Some(ResultBatch::with_rows(self.columns.clone(), rows)));
        }

        Ok(None)
    }

    fn columns(&self) -> &[String] {
        &self.columns
    }
}

// =============================================================================
// Empty Operator
// =============================================================================

struct EmptyOperator {
    done: bool,
}

impl EmptyOperator {
    fn new() -> Self {
        Self { done: false }
    }
}

impl Operator for EmptyOperator {
    fn next_batch(&mut self) -> ExecutorResult<Option<ResultBatch>> {
        if self.done {
            return Ok(None);
        }
        self.done = true;
        Ok(Some(ResultBatch::with_rows(vec![], vec![Row { values: vec![] }])))
    }

    fn columns(&self) -> &[String] {
        &[]
    }
}

// =============================================================================
// Expression Evaluation
// =============================================================================

fn evaluate_expression(
    expr: &PlanExpression,
    row: &Row,
    columns: &[String],
) -> ExecutorResult<Value> {
    match expr {
        PlanExpression::Literal(lit) => Ok(match lit {
            PlanLiteral::Null => Value::Null,
            PlanLiteral::Boolean(b) => Value::Boolean(*b),
            PlanLiteral::Integer(i) => Value::Integer(*i),
            PlanLiteral::Float(f) => Value::Float(*f),
            PlanLiteral::String(s) => Value::String(s.clone()),
        }),

        PlanExpression::Column { table: _, name, .. } => {
            if name == "*" {
                return Ok(Value::Null);
            }

            let idx = columns
                .iter()
                .position(|c| c == name)
                .ok_or_else(|| ExecutorError::ColumnNotFound(name.clone()))?;

            Ok(row.values.get(idx).cloned().unwrap_or(Value::Null))
        }

        PlanExpression::BinaryOp { left, op, right } => {
            let left_val = evaluate_expression(left, row, columns)?;
            let right_val = evaluate_expression(right, row, columns)?;
            evaluate_binary_op(*op, &left_val, &right_val)
        }

        PlanExpression::UnaryOp { op, expr } => {
            let val = evaluate_expression(expr, row, columns)?;
            evaluate_unary_op(*op, &val)
        }

        PlanExpression::IsNull { expr, negated } => {
            let val = evaluate_expression(expr, row, columns)?;
            let is_null = matches!(val, Value::Null);
            Ok(Value::Boolean(if *negated { !is_null } else { is_null }))
        }

        PlanExpression::Cast { expr, target_type } => {
            let val = evaluate_expression(expr, row, columns)?;
            cast_value(&val, target_type)
        }

        PlanExpression::Function { name, args, .. } => {
            let arg_values: Vec<Value> = args
                .iter()
                .map(|a| evaluate_expression(a, row, columns))
                .collect::<ExecutorResult<Vec<_>>>()?;
            evaluate_function(name, &arg_values)
        }
    }
}

fn evaluate_binary_op(op: PlanBinaryOp, left: &Value, right: &Value) -> ExecutorResult<Value> {
    if matches!(left, Value::Null) || matches!(right, Value::Null) {
        if matches!(op, PlanBinaryOp::Equal | PlanBinaryOp::NotEqual) {
            return Ok(Value::Null);
        }
        return Ok(Value::Null);
    }

    match op {
        PlanBinaryOp::Add => {
            let l = value_to_f64(left)?;
            let r = value_to_f64(right)?;
            Ok(Value::Float(l + r))
        }
        PlanBinaryOp::Subtract => {
            let l = value_to_f64(left)?;
            let r = value_to_f64(right)?;
            Ok(Value::Float(l - r))
        }
        PlanBinaryOp::Multiply => {
            let l = value_to_f64(left)?;
            let r = value_to_f64(right)?;
            Ok(Value::Float(l * r))
        }
        PlanBinaryOp::Divide => {
            let l = value_to_f64(left)?;
            let r = value_to_f64(right)?;
            if r == 0.0 {
                return Err(ExecutorError::DivisionByZero);
            }
            Ok(Value::Float(l / r))
        }
        PlanBinaryOp::Modulo => {
            let l = value_to_i64(left)?;
            let r = value_to_i64(right)?;
            if r == 0 {
                return Err(ExecutorError::DivisionByZero);
            }
            Ok(Value::Integer(l % r))
        }
        PlanBinaryOp::Equal => Ok(Value::Boolean(compare_values(left, right)? == std::cmp::Ordering::Equal)),
        PlanBinaryOp::NotEqual => Ok(Value::Boolean(compare_values(left, right)? != std::cmp::Ordering::Equal)),
        PlanBinaryOp::LessThan => Ok(Value::Boolean(compare_values(left, right)? == std::cmp::Ordering::Less)),
        PlanBinaryOp::LessThanOrEqual => Ok(Value::Boolean(compare_values(left, right)? != std::cmp::Ordering::Greater)),
        PlanBinaryOp::GreaterThan => Ok(Value::Boolean(compare_values(left, right)? == std::cmp::Ordering::Greater)),
        PlanBinaryOp::GreaterThanOrEqual => Ok(Value::Boolean(compare_values(left, right)? != std::cmp::Ordering::Less)),
        PlanBinaryOp::And => {
            let l = value_to_bool(left)?;
            let r = value_to_bool(right)?;
            Ok(Value::Boolean(l && r))
        }
        PlanBinaryOp::Or => {
            let l = value_to_bool(left)?;
            let r = value_to_bool(right)?;
            Ok(Value::Boolean(l || r))
        }
        PlanBinaryOp::Concat => {
            let l = value_to_string(left);
            let r = value_to_string(right);
            Ok(Value::String(format!("{}{}", l, r)))
        }
    }
}

fn evaluate_unary_op(op: PlanUnaryOp, value: &Value) -> ExecutorResult<Value> {
    match op {
        PlanUnaryOp::Not => {
            let b = value_to_bool(value)?;
            Ok(Value::Boolean(!b))
        }
        PlanUnaryOp::Negative => {
            let f = value_to_f64(value)?;
            Ok(Value::Float(-f))
        }
    }
}

fn evaluate_function(name: &str, args: &[Value]) -> ExecutorResult<Value> {
    match name.to_uppercase().as_str() {
        "UPPER" => {
            let s = value_to_string(&args.first().cloned().unwrap_or(Value::Null));
            Ok(Value::String(s.to_uppercase()))
        }
        "LOWER" => {
            let s = value_to_string(&args.first().cloned().unwrap_or(Value::Null));
            Ok(Value::String(s.to_lowercase()))
        }
        "LENGTH" => {
            let s = value_to_string(&args.first().cloned().unwrap_or(Value::Null));
            Ok(Value::Integer(s.len() as i64))
        }
        "ABS" => {
            let f = value_to_f64(&args.first().cloned().unwrap_or(Value::Null))?;
            Ok(Value::Float(f.abs()))
        }
        "COALESCE" => {
            for arg in args {
                if !matches!(arg, Value::Null) {
                    return Ok(arg.clone());
                }
            }
            Ok(Value::Null)
        }
        _ => Err(ExecutorError::InvalidOperation(format!(
            "Unknown function: {}",
            name
        ))),
    }
}

// =============================================================================
// Value Conversion Utilities
// =============================================================================

fn value_to_f64(value: &Value) -> ExecutorResult<f64> {
    match value {
        Value::Integer(i) => Ok(*i as f64),
        Value::Float(f) => Ok(*f),
        Value::String(s) => s.parse().map_err(|_| ExecutorError::TypeMismatch {
            expected: "number".to_string(),
            actual: "string".to_string(),
        }),
        _ => Err(ExecutorError::TypeMismatch {
            expected: "number".to_string(),
            actual: format!("{:?}", value),
        }),
    }
}

fn value_to_i64(value: &Value) -> ExecutorResult<i64> {
    match value {
        Value::Integer(i) => Ok(*i),
        Value::Float(f) => Ok(*f as i64),
        Value::String(s) => s.parse().map_err(|_| ExecutorError::TypeMismatch {
            expected: "integer".to_string(),
            actual: "string".to_string(),
        }),
        _ => Err(ExecutorError::TypeMismatch {
            expected: "integer".to_string(),
            actual: format!("{:?}", value),
        }),
    }
}

fn value_to_bool(value: &Value) -> ExecutorResult<bool> {
    match value {
        Value::Boolean(b) => Ok(*b),
        Value::Integer(i) => Ok(*i != 0),
        Value::Null => Ok(false),
        _ => Err(ExecutorError::TypeMismatch {
            expected: "boolean".to_string(),
            actual: format!("{:?}", value),
        }),
    }
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Integer(i) => i.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Boolean(b) => b.to_string(),
        Value::Null => String::new(),
        _ => format!("{:?}", value),
    }
}

fn compare_values(left: &Value, right: &Value) -> ExecutorResult<std::cmp::Ordering> {
    match (left, right) {
        (Value::Integer(l), Value::Integer(r)) => Ok(l.cmp(r)),
        (Value::Float(l), Value::Float(r)) => {
            Ok(l.partial_cmp(r).unwrap_or(std::cmp::Ordering::Equal))
        }
        (Value::Integer(l), Value::Float(r)) => {
            let l = *l as f64;
            Ok(l.partial_cmp(r).unwrap_or(std::cmp::Ordering::Equal))
        }
        (Value::Float(l), Value::Integer(r)) => {
            let r = *r as f64;
            Ok(l.partial_cmp(&r).unwrap_or(std::cmp::Ordering::Equal))
        }
        (Value::String(l), Value::String(r)) => Ok(l.cmp(r)),
        (Value::Boolean(l), Value::Boolean(r)) => Ok(l.cmp(r)),
        _ => Ok(std::cmp::Ordering::Equal),
    }
}

fn cast_value(value: &Value, target_type: &DataType) -> ExecutorResult<Value> {
    match target_type {
        DataType::Integer => Ok(Value::Integer(value_to_i64(value)?)),
        DataType::Float => Ok(Value::Float(value_to_f64(value)?)),
        DataType::Text => Ok(Value::String(value_to_string(value))),
        DataType::Boolean => Ok(Value::Boolean(value_to_bool(value)?)),
        _ => Ok(value.clone()),
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::{PlanNode, ProjectNode, ProjectionExpr, QueryPlan, ScanNode};

    fn create_test_context() -> ExecutionContext {
        let mut context = ExecutionContext::new();

        context.add_table(TableData {
            name: "users".to_string(),
            columns: vec![
                "id".to_string(),
                "name".to_string(),
                "age".to_string(),
            ],
            rows: vec![
                Row {
                    values: vec![
                        Value::Integer(1),
                        Value::String("Alice".to_string()),
                        Value::Integer(30),
                    ],
                },
                Row {
                    values: vec![
                        Value::Integer(2),
                        Value::String("Bob".to_string()),
                        Value::Integer(25),
                    ],
                },
                Row {
                    values: vec![
                        Value::Integer(3),
                        Value::String("Charlie".to_string()),
                        Value::Integer(35),
                    ],
                },
            ],
        });

        context
    }

    #[test]
    fn test_scan_operator() {
        let context = create_test_context();
        let executor = Executor::new(context);

        let plan = QueryPlan {
            root: PlanNode::Project(ProjectNode {
                input: Box::new(PlanNode::Scan(ScanNode {
                    table_name: "users".to_string(),
                    alias: None,
                    columns: vec![
                        "id".to_string(),
                        "name".to_string(),
                        "age".to_string(),
                    ],
                    index_scan: None,
                })),
                expressions: vec![
                    ProjectionExpr {
                        expr: PlanExpression::Column {
                            table: None,
                            name: "id".to_string(),
                            data_type: DataType::Integer,
                        },
                        alias: Some("id".to_string()),
                    },
                    ProjectionExpr {
                        expr: PlanExpression::Column {
                            table: None,
                            name: "name".to_string(),
                            data_type: DataType::Text,
                        },
                        alias: Some("name".to_string()),
                    },
                ],
            }),
            estimated_cost: 100.0,
            estimated_rows: 3,
        };

        let result = executor.execute(&plan).unwrap();

        assert_eq!(result.rows.len(), 3);
        assert_eq!(result.columns.len(), 2);
    }

    #[test]
    fn test_filter_operator() {
        let context = create_test_context();
        let executor = Executor::new(context);

        let plan = QueryPlan {
            root: PlanNode::Project(ProjectNode {
                input: Box::new(PlanNode::Filter(crate::planner::FilterNode {
                    input: Box::new(PlanNode::Scan(ScanNode {
                        table_name: "users".to_string(),
                        alias: None,
                        columns: vec![
                            "id".to_string(),
                            "name".to_string(),
                            "age".to_string(),
                        ],
                        index_scan: None,
                    })),
                    predicate: PlanExpression::BinaryOp {
                        left: Box::new(PlanExpression::Column {
                            table: None,
                            name: "age".to_string(),
                            data_type: DataType::Integer,
                        }),
                        op: PlanBinaryOp::GreaterThan,
                        right: Box::new(PlanExpression::Literal(PlanLiteral::Integer(28))),
                    },
                })),
                expressions: vec![ProjectionExpr {
                    expr: PlanExpression::Column {
                        table: None,
                        name: "name".to_string(),
                        data_type: DataType::Text,
                    },
                    alias: Some("name".to_string()),
                }],
            }),
            estimated_cost: 100.0,
            estimated_rows: 2,
        };

        let result = executor.execute(&plan).unwrap();

        assert_eq!(result.rows.len(), 2);
    }

    #[test]
    fn test_limit_operator() {
        let context = create_test_context();
        let executor = Executor::new(context);

        let plan = QueryPlan {
            root: PlanNode::Limit(LimitNode {
                input: Box::new(PlanNode::Project(ProjectNode {
                    input: Box::new(PlanNode::Scan(ScanNode {
                        table_name: "users".to_string(),
                        alias: None,
                        columns: vec!["id".to_string()],
                        index_scan: None,
                    })),
                    expressions: vec![ProjectionExpr {
                        expr: PlanExpression::Column {
                            table: None,
                            name: "id".to_string(),
                            data_type: DataType::Integer,
                        },
                        alias: Some("id".to_string()),
                    }],
                })),
                limit: Some(2),
                offset: None,
            }),
            estimated_cost: 100.0,
            estimated_rows: 2,
        };

        let result = executor.execute(&plan).unwrap();

        assert_eq!(result.rows.len(), 2);
    }
}
