//! Shared admission for boundary-decided predicate materialization: locate
//! the named ordering `MaterializeBoolean*`, confirm its clean
//! `[def result]` shape, resolve every implicit flag unit it reads to the
//! same compare definition — in-block or across predecessor edges — and
//! prove one compare operand sits at a carrier-domain pole that fixes the
//! predicate, or collapses it to equality, without consulting the other.
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::member_position;
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use semantic_vocabulary::IntegerValue;
