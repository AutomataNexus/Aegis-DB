# AegisQL - Query Language Reference

**Version:** 1.0.0
**Last Updated:** January 2026

---

## Table of Contents

1. [Introduction](#1-introduction)
2. [Quick Start](#2-quick-start)
3. [Data Types](#3-data-types)
4. [DDL - Data Definition Language](#4-ddl---data-definition-language)
5. [DML - Data Manipulation Language](#5-dml---data-manipulation-language)
6. [Expressions and Operators](#6-expressions-and-operators)
7. [Built-in Functions](#7-built-in-functions)
8. [Joins](#8-joins)
9. [Subqueries](#9-subqueries)
10. [Transactions](#10-transactions)
11. [Time Series Extensions](#11-time-series-extensions)
12. [Document Store Queries](#12-document-store-queries)
13. [Streaming Queries](#13-streaming-queries)
14. [Performance Optimization](#14-performance-optimization)
15. [Error Handling](#15-error-handling)
16. [Examples](#16-examples)

---

## 1. Introduction

AegisQL is the query language for the Aegis Database Platform. It extends standard SQL with specialized syntax for time series data, document operations, and streaming queries while maintaining full ANSI SQL compatibility for relational workloads.

### Key Features

- **ANSI SQL Compatible**: Full support for standard SQL syntax
- **Multi-Paradigm**: Unified queries across relational, time series, document, and streaming data
- **Type Safe**: Strong typing with automatic type coercion where appropriate
- **Optimized Execution**: Volcano-style iterator model with vectorized batch processing

### Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        AegisQL Query                         │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                         Parser                               │
│              (sqlparser-rs + Aegis extensions)               │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    Abstract Syntax Tree                      │
│                   (Typed AST Representation)                 │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                        Analyzer                              │
│          (Semantic validation, type checking)                │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                        Planner                               │
│     (Query optimization, cost-based planning)                │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                        Executor                              │
│        (Volcano iterator model, vectorized batches)          │
└─────────────────────────────────────────────────────────────┘
```

---

## 2. Quick Start

### Connecting

```bash
# Via CLI - using shorthand
aegis-client -d nexusscribe shell

# Via CLI - using server URL
aegis-client -s http://localhost:9090 shell

# Via CLI - execute query directly
aegis-client -d nexusscribe query "SELECT * FROM users LIMIT 10"

# Via REST API
curl -X POST http://localhost:9090/api/v1/query \
  -H "Content-Type: application/json" \
  -d '{"sql": "SELECT * FROM users LIMIT 10", "params": []}'
```

**CLI Shorthand Names:**
| Name | Port |
|------|------|
| `dashboard` | 9090 |
| `axonml` | 7001 |
| `nexusscribe` | 9091 |

### Basic Queries

```sql
-- Create a table
CREATE TABLE users (
    id INTEGER NOT NULL,
    name VARCHAR(255),
    email VARCHAR(255),
    created_at TIMESTAMP
);

-- Insert data
INSERT INTO users (id, name, email)
VALUES (1, 'Alice', 'alice@example.com');

-- Query data
SELECT * FROM users WHERE name = 'Alice';

-- Update data
UPDATE users SET email = 'alice@newdomain.com' WHERE id = 1;

-- Delete data
DELETE FROM users WHERE id = 1;
```

---

## 3. Data Types

### Numeric Types

| Type | Description | Range | Storage |
|------|-------------|-------|---------|
| `TINYINT` | Tiny integer | -128 to 127 | 1 byte |
| `SMALLINT` | Small integer | -32,768 to 32,767 | 2 bytes |
| `INTEGER` / `INT` | Standard integer | -2^31 to 2^31-1 | 4 bytes |
| `BIGINT` | Large integer | -2^63 to 2^63-1 | 8 bytes |
| `REAL` / `FLOAT` | Single precision | IEEE 754 | 4 bytes |
| `DOUBLE` / `DOUBLE PRECISION` | Double precision | IEEE 754 | 8 bytes |
| `DECIMAL(p,s)` / `NUMERIC(p,s)` | Exact decimal | User-defined precision | Variable |

### String Types

| Type | Description | Max Length |
|------|-------------|------------|
| `CHAR(n)` | Fixed-length string | n characters |
| `VARCHAR(n)` | Variable-length string | n characters |
| `TEXT` | Unlimited text | No limit |

### Date/Time Types

| Type | Description | Format |
|------|-------------|--------|
| `DATE` | Calendar date | YYYY-MM-DD |
| `TIME` | Time of day | HH:MM:SS |
| `TIMESTAMP` | Date and time | YYYY-MM-DD HH:MM:SS |

### Other Types

| Type | Description |
|------|-------------|
| `BOOLEAN` | True/False |
| `BLOB` | Binary large object |
| `JSON` | JSON document |
| `UUID` | Universally unique identifier |

### Type Examples

```sql
CREATE TABLE example_types (
    id BIGINT NOT NULL,
    name VARCHAR(100),
    balance DECIMAL(10, 2),
    is_active BOOLEAN,
    metadata JSON,
    created_at TIMESTAMP,
    profile_id UUID
);
```

---

## 4. DDL - Data Definition Language

### CREATE TABLE

```sql
CREATE TABLE [IF NOT EXISTS] table_name (
    column_name data_type [constraints],
    ...
    [table_constraints]
);
```

**Column Constraints:**
- `NOT NULL` - Column cannot contain NULL values
- `DEFAULT value` - Default value for the column
- `PRIMARY KEY` - Column is the primary key
- `UNIQUE` - All values must be unique
- `REFERENCES table(column)` - Foreign key reference
- `CHECK (expression)` - Check constraint

**Examples:**

```sql
-- Basic table
CREATE TABLE products (
    id INTEGER NOT NULL,
    name VARCHAR(255) NOT NULL,
    price DECIMAL(10, 2) DEFAULT 0.00,
    stock INTEGER DEFAULT 0
);

-- With constraints
CREATE TABLE orders (
    id INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    product_id INTEGER NOT NULL,
    quantity INTEGER NOT NULL,
    total DECIMAL(10, 2),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- If not exists
CREATE TABLE IF NOT EXISTS logs (
    id BIGINT NOT NULL,
    message TEXT,
    level VARCHAR(10),
    timestamp TIMESTAMP
);
```

### DROP TABLE

```sql
DROP TABLE [IF EXISTS] table_name;
```

**Examples:**

```sql
DROP TABLE products;
DROP TABLE IF EXISTS temp_data;
```

### ALTER TABLE

```sql
ALTER TABLE table_name operation [, operation, ...];
```

**Operations:**

| Operation | Syntax |
|-----------|--------|
| Add Column | `ADD COLUMN column_name data_type [constraints]` |
| Drop Column | `DROP COLUMN [IF EXISTS] column_name` |
| Rename Column | `RENAME COLUMN old_name TO new_name` |
| Alter Column Type | `ALTER COLUMN column_name SET DATA TYPE data_type` |
| Set Not Null | `ALTER COLUMN column_name SET NOT NULL` |
| Drop Not Null | `ALTER COLUMN column_name DROP NOT NULL` |
| Set Default | `ALTER COLUMN column_name SET DEFAULT expression` |
| Drop Default | `ALTER COLUMN column_name DROP DEFAULT` |
| Rename Table | `RENAME TO new_table_name` |
| Drop Constraint | `DROP CONSTRAINT constraint_name` |

**Examples:**

```sql
-- Add a column
ALTER TABLE users ADD COLUMN phone VARCHAR(20);

-- Add column with constraints
ALTER TABLE users ADD COLUMN status VARCHAR(10) NOT NULL DEFAULT 'active';

-- Drop a column
ALTER TABLE users DROP COLUMN phone;

-- Drop column if exists
ALTER TABLE users DROP COLUMN IF EXISTS temp_field;

-- Rename a column
ALTER TABLE users RENAME COLUMN name TO full_name;

-- Change column data type
ALTER TABLE products ALTER COLUMN price SET DATA TYPE DECIMAL(12, 2);

-- Set column to NOT NULL
ALTER TABLE orders ALTER COLUMN customer_id SET NOT NULL;

-- Remove NOT NULL constraint
ALTER TABLE orders ALTER COLUMN notes DROP NOT NULL;

-- Set default value
ALTER TABLE users ALTER COLUMN created_at SET DEFAULT CURRENT_TIMESTAMP;

-- Drop default value
ALTER TABLE users ALTER COLUMN status DROP DEFAULT;

-- Rename table
ALTER TABLE users RENAME TO customers;

-- Multiple operations
ALTER TABLE products
    ADD COLUMN category VARCHAR(50),
    ADD COLUMN updated_at TIMESTAMP,
    DROP COLUMN old_field;
```

### CREATE INDEX

```sql
CREATE [UNIQUE] INDEX [IF NOT EXISTS] index_name
ON table_name (column1 [, column2, ...]);
```

**Examples:**

```sql
-- Simple index
CREATE INDEX idx_users_email ON users (email);

-- Composite index
CREATE INDEX idx_orders_user_date ON orders (user_id, created_at);

-- Unique index
CREATE UNIQUE INDEX idx_users_username ON users (username);

-- If not exists
CREATE INDEX IF NOT EXISTS idx_products_name ON products (name);
```

### DROP INDEX

```sql
DROP INDEX [IF EXISTS] index_name;
```

---

## 5. DML - Data Manipulation Language

### SELECT

```sql
SELECT [DISTINCT] select_list
FROM table_reference
[WHERE condition]
[GROUP BY expression_list]
[HAVING condition]
[ORDER BY order_list]
[LIMIT count]
[OFFSET start];
```

**Select List Options:**
- `*` - All columns
- `table.*` - All columns from specific table
- `column` - Single column
- `column AS alias` - Column with alias
- `expression AS alias` - Computed expression

**Examples:**

```sql
-- Select all columns
SELECT * FROM users;

-- Select specific columns
SELECT id, name, email FROM users;

-- With alias
SELECT
    u.name AS user_name,
    COUNT(*) AS order_count
FROM users u
JOIN orders o ON u.id = o.user_id
GROUP BY u.name;

-- Distinct values
SELECT DISTINCT status FROM orders;

-- With WHERE clause
SELECT * FROM products WHERE price > 100 AND stock > 0;

-- With ORDER BY
SELECT * FROM users ORDER BY created_at DESC;

-- With LIMIT and OFFSET
SELECT * FROM products ORDER BY price DESC LIMIT 10 OFFSET 20;

-- With GROUP BY and HAVING
SELECT
    category,
    AVG(price) AS avg_price
FROM products
GROUP BY category
HAVING AVG(price) > 50;
```

### INSERT

```sql
-- Insert with column list
INSERT INTO table_name (column1, column2, ...)
VALUES (value1, value2, ...);

-- Insert multiple rows
INSERT INTO table_name (column1, column2, ...)
VALUES
    (value1a, value2a, ...),
    (value1b, value2b, ...);

-- Insert from SELECT
INSERT INTO table_name (column1, column2, ...)
SELECT col1, col2, ... FROM other_table WHERE condition;
```

**Examples:**

```sql
-- Single row
INSERT INTO users (id, name, email)
VALUES (1, 'Alice', 'alice@example.com');

-- Multiple rows
INSERT INTO products (id, name, price) VALUES
    (1, 'Widget', 9.99),
    (2, 'Gadget', 19.99),
    (3, 'Gizmo', 29.99);

-- From SELECT
INSERT INTO order_archive (id, user_id, total, created_at)
SELECT id, user_id, total, created_at
FROM orders
WHERE created_at < '2024-01-01';
```

### UPDATE

```sql
UPDATE table_name
SET column1 = value1 [, column2 = value2, ...]
[WHERE condition];
```

**Examples:**

```sql
-- Update single column
UPDATE users SET email = 'newemail@example.com' WHERE id = 1;

-- Update multiple columns
UPDATE products
SET price = price * 1.1, updated_at = CURRENT_TIMESTAMP
WHERE category = 'electronics';

-- Update all rows (careful!)
UPDATE products SET stock = 0;
```

### DELETE

```sql
DELETE FROM table_name [WHERE condition];
```

**Examples:**

```sql
-- Delete specific rows
DELETE FROM orders WHERE status = 'cancelled';

-- Delete with subquery condition
DELETE FROM users WHERE id NOT IN (SELECT DISTINCT user_id FROM orders);

-- Delete all rows (careful!)
DELETE FROM temp_data;
```

---

## 6. Expressions and Operators

### Arithmetic Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `+` | Addition | `price + tax` |
| `-` | Subtraction | `total - discount` |
| `*` | Multiplication | `quantity * price` |
| `/` | Division | `total / count` |
| `%` | Modulo | `id % 10` |

### Comparison Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `=` | Equal | `status = 'active'` |
| `<>` or `!=` | Not equal | `status <> 'deleted'` |
| `<` | Less than | `age < 18` |
| `<=` | Less than or equal | `price <= 100` |
| `>` | Greater than | `stock > 0` |
| `>=` | Greater than or equal | `rating >= 4` |

### Logical Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `AND` | Logical AND | `a > 5 AND b < 10` |
| `OR` | Logical OR | `status = 'active' OR status = 'pending'` |
| `NOT` | Logical NOT | `NOT is_deleted` |

### Special Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `IS NULL` | Check for NULL | `deleted_at IS NULL` |
| `IS NOT NULL` | Check for non-NULL | `email IS NOT NULL` |
| `IN` | Match any in list | `status IN ('active', 'pending')` |
| `NOT IN` | Not in list | `id NOT IN (1, 2, 3)` |
| `BETWEEN` | Range check | `price BETWEEN 10 AND 100` |
| `NOT BETWEEN` | Outside range | `age NOT BETWEEN 18 AND 65` |
| `LIKE` | Pattern matching | `name LIKE 'A%'` |
| `NOT LIKE` | Pattern not matching | `email NOT LIKE '%@test.com'` |

### LIKE Pattern Syntax

| Pattern | Description |
|---------|-------------|
| `%` | Matches any sequence of characters |
| `_` | Matches any single character |

**Examples:**

```sql
-- Starts with 'A'
SELECT * FROM users WHERE name LIKE 'A%';

-- Ends with '.com'
SELECT * FROM users WHERE email LIKE '%.com';

-- Contains 'smith'
SELECT * FROM users WHERE name LIKE '%smith%';

-- Exactly 5 characters
SELECT * FROM codes WHERE code LIKE '_____';

-- Second character is 'a'
SELECT * FROM products WHERE name LIKE '_a%';
```

### String Concatenation

```sql
-- Using || operator
SELECT first_name || ' ' || last_name AS full_name FROM users;
```

---

## 7. Built-in Functions

### Aggregate Functions

| Function | Description | Example |
|----------|-------------|---------|
| `COUNT(*)` | Count all rows | `SELECT COUNT(*) FROM users` |
| `COUNT(column)` | Count non-NULL values | `SELECT COUNT(email) FROM users` |
| `SUM(column)` | Sum of values | `SELECT SUM(amount) FROM orders` |
| `AVG(column)` | Average of values | `SELECT AVG(price) FROM products` |
| `MIN(column)` | Minimum value | `SELECT MIN(created_at) FROM users` |
| `MAX(column)` | Maximum value | `SELECT MAX(price) FROM products` |

**Examples:**

```sql
-- Multiple aggregates
SELECT
    COUNT(*) AS total_orders,
    SUM(total) AS revenue,
    AVG(total) AS avg_order_value,
    MIN(total) AS smallest_order,
    MAX(total) AS largest_order
FROM orders
WHERE status = 'completed';

-- With GROUP BY
SELECT
    category,
    COUNT(*) AS product_count,
    AVG(price) AS avg_price
FROM products
GROUP BY category
ORDER BY product_count DESC;
```

### String Functions

| Function | Description | Example |
|----------|-------------|---------|
| `UPPER(str)` | Convert to uppercase | `UPPER('hello')` → `'HELLO'` |
| `LOWER(str)` | Convert to lowercase | `LOWER('HELLO')` → `'hello'` |
| `LENGTH(str)` | String length | `LENGTH('hello')` → `5` |

**Examples:**

```sql
SELECT
    UPPER(name) AS upper_name,
    LOWER(email) AS lower_email,
    LENGTH(description) AS desc_length
FROM products;
```

### Numeric Functions

| Function | Description | Example |
|----------|-------------|---------|
| `ABS(num)` | Absolute value | `ABS(-5)` → `5` |

### NULL Handling Functions

| Function | Description | Example |
|----------|-------------|---------|
| `COALESCE(a, b, ...)` | First non-NULL value | `COALESCE(nickname, name, 'Anonymous')` |

**Examples:**

```sql
-- Use default if NULL
SELECT
    name,
    COALESCE(phone, 'No phone') AS contact_phone
FROM users;

-- Chain multiple fallbacks
SELECT COALESCE(preferred_name, first_name, username, 'Guest') AS display_name
FROM users;
```

---

## 8. Joins

### Join Types

| Type | Description |
|------|-------------|
| `INNER JOIN` | Rows matching in both tables |
| `LEFT JOIN` / `LEFT OUTER JOIN` | All rows from left, matching from right |
| `RIGHT JOIN` / `RIGHT OUTER JOIN` | All rows from right, matching from left |
| `FULL JOIN` / `FULL OUTER JOIN` | All rows from both tables |
| `CROSS JOIN` | Cartesian product |

### Join Syntax

```sql
SELECT columns
FROM table1
[INNER|LEFT|RIGHT|FULL] JOIN table2 ON condition
[JOIN table3 ON condition]
...
```

### Examples

```sql
-- Inner Join
SELECT
    u.name AS user_name,
    o.id AS order_id,
    o.total
FROM users u
INNER JOIN orders o ON u.id = o.user_id;

-- Left Join (include users without orders)
SELECT
    u.name,
    COUNT(o.id) AS order_count
FROM users u
LEFT JOIN orders o ON u.id = o.user_id
GROUP BY u.name;

-- Multiple Joins
SELECT
    u.name AS customer,
    p.name AS product,
    oi.quantity,
    oi.price
FROM users u
JOIN orders o ON u.id = o.user_id
JOIN order_items oi ON o.id = oi.order_id
JOIN products p ON oi.product_id = p.id
WHERE o.status = 'completed';

-- Self Join (hierarchical data)
SELECT
    e.name AS employee,
    m.name AS manager
FROM employees e
LEFT JOIN employees m ON e.manager_id = m.id;

-- Cross Join (all combinations)
SELECT
    c.name AS color,
    s.name AS size
FROM colors c
CROSS JOIN sizes s;
```

---

## 9. Subqueries

### Subquery Locations

Subqueries can appear in:
- `SELECT` list (scalar subquery)
- `FROM` clause (derived table)
- `WHERE` clause (filter condition)
- `IN` / `NOT IN` expressions
- `EXISTS` / `NOT EXISTS` checks

### Examples

```sql
-- Scalar subquery in SELECT
SELECT
    name,
    (SELECT COUNT(*) FROM orders WHERE orders.user_id = users.id) AS order_count
FROM users;

-- Subquery in FROM (derived table)
SELECT
    category,
    avg_price
FROM (
    SELECT
        category,
        AVG(price) AS avg_price
    FROM products
    GROUP BY category
) AS category_stats
WHERE avg_price > 50;

-- Subquery in WHERE with IN
SELECT * FROM users
WHERE id IN (
    SELECT DISTINCT user_id
    FROM orders
    WHERE total > 1000
);

-- Subquery with NOT IN
SELECT * FROM products
WHERE id NOT IN (
    SELECT product_id FROM order_items
);

-- Correlated subquery
SELECT * FROM products p
WHERE price > (
    SELECT AVG(price)
    FROM products
    WHERE category = p.category
);
```

---

## 10. Transactions

### Transaction Control

```sql
-- Start a transaction
BEGIN;

-- Commit changes
COMMIT;

-- Rollback changes
ROLLBACK;
```

### ACID Properties

Aegis provides full ACID transaction support:

- **Atomicity**: All operations succeed or all fail
- **Consistency**: Database remains in valid state
- **Isolation**: Transactions don't interfere (MVCC with snapshot isolation)
- **Durability**: Committed changes survive failures (WAL)

### Examples

```sql
-- Transfer funds between accounts
BEGIN;

UPDATE accounts SET balance = balance - 100 WHERE id = 1;
UPDATE accounts SET balance = balance + 100 WHERE id = 2;

-- Verify balances are valid
-- If any check fails, rollback

COMMIT;
```

```sql
-- Batch insert with rollback on error
BEGIN;

INSERT INTO orders (id, user_id, total) VALUES (1001, 5, 150.00);
INSERT INTO order_items (order_id, product_id, quantity) VALUES (1001, 10, 2);
INSERT INTO order_items (order_id, product_id, quantity) VALUES (1001, 15, 1);

-- If everything succeeded
COMMIT;

-- If any error occurred
-- ROLLBACK;
```

---

## 11. Time Series Extensions

Aegis provides specialized support for time series data with optimized storage and query capabilities.

### Time Series Query API

```rust
// Rust SDK example
let query = TimeSeriesQuery::last("cpu_usage", Duration::hours(1))
    .with_tags(tags!("host" => "server1"))
    .downsample(Duration::minutes(5), AggregateFunction::Avg)
    .with_limit(100);
```

### REST API

```bash
# Query time series data
curl -X POST http://localhost:9090/api/v1/timeseries/query \
  -H "Content-Type: application/json" \
  -d '{
    "metric": "cpu_usage",
    "start": "2024-01-01T00:00:00Z",
    "end": "2024-01-02T00:00:00Z",
    "tags": {"host": "server1"},
    "aggregation": {
      "type": "downsample",
      "interval": "5m",
      "function": "avg"
    }
  }'
```

### Aggregation Functions

| Function | Description |
|----------|-------------|
| `AVG` | Average value |
| `SUM` | Sum of values |
| `MIN` | Minimum value |
| `MAX` | Maximum value |
| `COUNT` | Number of points |
| `FIRST` | First value in window |
| `LAST` | Last value in window |
| `STDDEV` | Standard deviation |
| `PERCENTILE_50` | 50th percentile (median) |
| `PERCENTILE_90` | 90th percentile |
| `PERCENTILE_99` | 99th percentile |

### Downsampling

Reduce data resolution for long-term storage or visualization:

```rust
// Downsample to 5-minute averages
query.downsample(Duration::minutes(5), AggregateFunction::Avg)

// Downsample to hourly maximums
query.downsample(Duration::hours(1), AggregateFunction::Max)
```

### Rolling Windows

Compute rolling aggregations:

```rust
// 10-minute rolling average
query.with_aggregation(QueryAggregation::RollingWindow {
    window: Duration::minutes(10),
    function: AggregateFunction::Avg,
})
```

### Retention Policies

```rust
// Configure retention
let policy = RetentionPolicy::new(Duration::days(30))
    .with_downsample_after(Duration::days(7), Duration::hours(1));
```

---

## 12. Document Store Queries

Aegis provides MongoDB-style document operations with full JSON support.

### Creating Collections

```rust
// Create a collection with schema validation
let schema = json!({
    "type": "object",
    "required": ["name", "email"],
    "properties": {
        "name": {"type": "string"},
        "email": {"type": "string", "format": "email"},
        "age": {"type": "integer", "minimum": 0}
    }
});

engine.create_collection("users", Some(schema))?;
```

### Document Operations

```rust
// Insert document
let doc = json!({
    "name": "Alice",
    "email": "alice@example.com",
    "age": 30,
    "tags": ["developer", "rust"]
});
let id = engine.insert("users", doc)?;

// Find by ID
let doc = engine.find_by_id("users", &id)?;

// Update document
engine.update("users", &id, json!({
    "$set": {"age": 31}
}))?;

// Delete document
engine.delete("users", &id)?;
```

### Query Syntax

```rust
// Find with filter
let filter = json!({
    "age": {"$gte": 18},
    "status": "active"
});
let docs = engine.find("users", filter)?;

// Complex query
let filter = json!({
    "$and": [
        {"age": {"$gte": 21}},
        {"$or": [
            {"role": "admin"},
            {"permissions": {"$contains": "write"}}
        ]}
    ]
});
```

### Query Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `$eq` | Equal | `{"age": {"$eq": 30}}` |
| `$ne` | Not equal | `{"status": {"$ne": "deleted"}}` |
| `$gt` | Greater than | `{"price": {"$gt": 100}}` |
| `$gte` | Greater or equal | `{"age": {"$gte": 18}}` |
| `$lt` | Less than | `{"stock": {"$lt": 10}}` |
| `$lte` | Less or equal | `{"rating": {"$lte": 5}}` |
| `$in` | In array | `{"status": {"$in": ["active", "pending"]}}` |
| `$nin` | Not in array | `{"role": {"$nin": ["banned", "suspended"]}}` |
| `$and` | Logical AND | `{"$and": [{...}, {...}]}` |
| `$or` | Logical OR | `{"$or": [{...}, {...}]}` |
| `$not` | Logical NOT | `{"$not": {"age": {"$lt": 18}}}` |
| `$exists` | Field exists | `{"email": {"$exists": true}}` |
| `$contains` | Array contains | `{"tags": {"$contains": "rust"}}` |
| `$regex` | Regex match | `{"name": {"$regex": "^A.*"}}` |

### REST API

```bash
# Query documents
curl -X POST http://localhost:9090/api/v1/documents/users/query \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {"age": {"$gte": 18}},
    "sort": {"name": 1},
    "limit": 10
  }'
```

---

## 13. Streaming Queries

Aegis supports real-time streaming with pub/sub and change data capture (CDC).

### Pub/Sub

```rust
// Subscribe to channel
let mut subscriber = engine.subscribe("orders")?;

while let Some(event) = subscriber.next().await {
    match event {
        Event::Message(msg) => println!("Received: {:?}", msg),
        Event::Error(e) => eprintln!("Error: {}", e),
    }
}

// Publish message
engine.publish("orders", json!({
    "order_id": 12345,
    "status": "created"
}))?;
```

### Change Data Capture (CDC)

```rust
// Subscribe to changes on a table
let cdc = engine.subscribe_cdc("users")?;

while let Some(change) = cdc.next().await {
    match change.operation {
        CdcOperation::Insert => println!("New user: {:?}", change.data),
        CdcOperation::Update => println!("Updated: {:?}", change.data),
        CdcOperation::Delete => println!("Deleted: {:?}", change.key),
    }
}
```

### WebSocket API

```javascript
// JavaScript client
const ws = new WebSocket('ws://localhost:9090/api/v1/streaming/orders');

ws.onmessage = (event) => {
    const data = JSON.parse(event.data);
    console.log('Received:', data);
};
```

---

## 14. Performance Optimization

### Query Optimization Tips

1. **Use Indexes**: Create indexes on frequently filtered columns
   ```sql
   CREATE INDEX idx_users_email ON users (email);
   CREATE INDEX idx_orders_user_date ON orders (user_id, created_at);
   ```

2. **Limit Results**: Always use LIMIT for large tables
   ```sql
   SELECT * FROM logs ORDER BY timestamp DESC LIMIT 100;
   ```

3. **Select Specific Columns**: Avoid `SELECT *` in production
   ```sql
   SELECT id, name, email FROM users;  -- Better
   SELECT * FROM users;                 -- Avoid
   ```

4. **Use WHERE Before GROUP BY**: Filter early to reduce processing
   ```sql
   SELECT category, COUNT(*)
   FROM products
   WHERE status = 'active'  -- Filter first
   GROUP BY category;
   ```

5. **Avoid Functions on Indexed Columns**:
   ```sql
   -- Bad: Can't use index on email
   SELECT * FROM users WHERE LOWER(email) = 'test@example.com';

   -- Better: Store normalized data
   SELECT * FROM users WHERE email_lower = 'test@example.com';
   ```

### Execution Plans

Aegis uses cost-based optimization to choose the best execution plan:

- **Scan**: Full table scan
- **Index Scan**: Use index for filtering
- **Nested Loop Join**: For small datasets
- **Hash Join**: For larger equi-joins

### Batch Processing

For bulk operations, use batch sizes:

```rust
// Configure batch size
let context = ExecutionContext::new()
    .with_batch_size(1024);  // Process 1024 rows at a time
```

---

## 15. Error Handling

### Common Error Types

| Error | Description | Solution |
|-------|-------------|----------|
| `ParseError` | Invalid SQL syntax | Check query syntax |
| `TableNotFound` | Table doesn't exist | Verify table name |
| `ColumnNotFound` | Column doesn't exist | Check column name in schema |
| `TypeMismatch` | Incompatible types | Cast values appropriately |
| `DivisionByZero` | Division by zero | Add null/zero checks |
| `ConstraintViolation` | Constraint failed | Check data constraints |

### Error Messages

```json
{
    "error": {
        "code": "PARSE_ERROR",
        "message": "Expected SELECT, INSERT, UPDATE, or DELETE",
        "position": 0,
        "sql": "SELEC * FROM users"
    }
}
```

---

## 16. Examples

### E-commerce Analytics

```sql
-- Top selling products this month
SELECT
    p.name,
    SUM(oi.quantity) AS units_sold,
    SUM(oi.quantity * oi.price) AS revenue
FROM products p
JOIN order_items oi ON p.id = oi.product_id
JOIN orders o ON oi.order_id = o.id
WHERE o.created_at >= '2024-01-01'
  AND o.status = 'completed'
GROUP BY p.id, p.name
ORDER BY revenue DESC
LIMIT 10;

-- Customer lifetime value
SELECT
    u.id,
    u.name,
    COUNT(DISTINCT o.id) AS total_orders,
    SUM(o.total) AS lifetime_value,
    AVG(o.total) AS avg_order_value
FROM users u
LEFT JOIN orders o ON u.id = o.user_id
WHERE o.status = 'completed'
GROUP BY u.id, u.name
ORDER BY lifetime_value DESC;
```

### User Analytics

```sql
-- Daily active users
SELECT
    DATE(last_active) AS date,
    COUNT(DISTINCT id) AS active_users
FROM users
WHERE last_active >= '2024-01-01'
GROUP BY DATE(last_active)
ORDER BY date;

-- User retention (users who ordered more than once)
SELECT
    COUNT(*) AS retained_users,
    (COUNT(*) * 100.0 / (SELECT COUNT(*) FROM users)) AS retention_rate
FROM (
    SELECT user_id
    FROM orders
    GROUP BY user_id
    HAVING COUNT(*) > 1
) AS repeat_customers;
```

### Inventory Management

```sql
-- Low stock alerts
SELECT
    name,
    stock,
    reorder_point
FROM products
WHERE stock <= reorder_point
ORDER BY (stock * 1.0 / reorder_point);

-- Stock value by category
SELECT
    category,
    SUM(stock) AS total_units,
    SUM(stock * cost) AS inventory_value
FROM products
GROUP BY category
ORDER BY inventory_value DESC;
```

### Time Series Examples

```rust
// Server monitoring - CPU usage over last hour
let cpu_query = TimeSeriesQuery::last("system.cpu.usage", Duration::hours(1))
    .with_tags(tags!("host" => "web-server-1"))
    .downsample(Duration::minutes(1), AggregateFunction::Avg);

// Memory usage with percentiles
let memory_query = TimeSeriesQuery::last("system.memory.used", Duration::hours(24))
    .downsample(Duration::hours(1), AggregateFunction::Percentile99);

// Network traffic
let network_query = TimeSeriesQuery::last("network.bytes.in", Duration::days(7))
    .with_tags(tags!("interface" => "eth0"))
    .downsample(Duration::hours(1), AggregateFunction::Sum);
```

---

## Appendix A: Reserved Keywords

```
ADD, ALL, ALTER, AND, AS, ASC, BETWEEN, BY, CASE, CAST,
CHECK, COLUMN, CONSTRAINT, CREATE, CROSS, DELETE, DESC,
DISTINCT, DROP, ELSE, END, EXISTS, FALSE, FOREIGN, FROM,
FULL, GROUP, HAVING, IF, IN, INDEX, INNER, INSERT, INTO,
IS, JOIN, KEY, LEFT, LIKE, LIMIT, NOT, NULL, OFFSET, ON,
OR, ORDER, OUTER, PRIMARY, REFERENCES, RIGHT, SELECT, SET,
TABLE, THEN, TRUE, UNION, UNIQUE, UPDATE, VALUES, WHEN,
WHERE, WITH
```

---

## Appendix B: SQL Grammar (BNF)

```bnf
<statement> ::= <select_stmt> | <insert_stmt> | <update_stmt> | <delete_stmt>
              | <create_table_stmt> | <drop_table_stmt> | <create_index_stmt>
              | <transaction_stmt>

<select_stmt> ::= SELECT [DISTINCT] <select_list>
                  [FROM <table_ref>]
                  [WHERE <expression>]
                  [GROUP BY <expr_list>]
                  [HAVING <expression>]
                  [ORDER BY <order_list>]
                  [LIMIT <integer>]
                  [OFFSET <integer>]

<insert_stmt> ::= INSERT INTO <table_name> [(<column_list>)]
                  VALUES <values_list>
                | INSERT INTO <table_name> [(<column_list>)]
                  <select_stmt>

<update_stmt> ::= UPDATE <table_name>
                  SET <assignment_list>
                  [WHERE <expression>]

<delete_stmt> ::= DELETE FROM <table_name>
                  [WHERE <expression>]

<expression> ::= <literal>
               | <column_ref>
               | <expression> <binary_op> <expression>
               | <unary_op> <expression>
               | <function_call>
               | (<expression>)
               | <expression> IS [NOT] NULL
               | <expression> [NOT] IN (<expr_list>)
               | <expression> [NOT] BETWEEN <expression> AND <expression>
               | <expression> [NOT] LIKE <pattern>
```

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | Jan 2026 | Initial release |

---

*Copyright 2024-2026 AutomataNexus Development Team. Licensed under Apache 2.0.*
