//! Record builders for constructing VRL-ready records from OTLP data.
//!
//! This module provides builders for logs, spans, and metrics that convert
//! decoded OTLP data into VRL Values for transformation.

mod log_builder;
mod metric_builder;
mod span_builder;

// Re-export public items used by other modules
pub use log_builder::{build_log_record, preallocate_log_values, LogRecordParts};
pub use metric_builder::{
    build_gauge_record, build_sum_record, preallocate_metric_values, ExemplarParts,
    GaugeRecordParts, SumRecordParts,
};
pub use span_builder::{
    build_span_record, preallocate_span_values, SpanEventParts, SpanLinkParts, SpanRecordParts,
};
