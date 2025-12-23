use bytes::Bytes;
use vrl::value::Value as VrlValue;

use super::common::{looks_like_json, DecodeError, DecodeFormat};
use super::metrics_json;
use super::metrics_proto;

/// Decode OTLP metrics (JSON or protobuf) into VRL Values, ready for transform.
pub fn decode_metrics(body: Bytes, format: DecodeFormat) -> Result<Vec<VrlValue>, DecodeError> {
    match format {
        DecodeFormat::Json => metrics_json::decode_json(&body),
        DecodeFormat::Protobuf => metrics_proto::decode_protobuf(&body),
        DecodeFormat::Auto => {
            if looks_like_json(&body) {
                match metrics_json::decode_json(&body) {
                    Ok(v) => Ok(v),
                    Err(json_err) => metrics_proto::decode_protobuf(&body).map_err(|proto_err| {
                        DecodeError::Unsupported(format!(
                            "json decode failed: {}; protobuf fallback failed: {}",
                            json_err, proto_err
                        ))
                    }),
                }
            } else {
                match metrics_proto::decode_protobuf(&body) {
                    Ok(v) => Ok(v),
                    Err(proto_err) => metrics_json::decode_json(&body).map_err(|json_err| {
                        DecodeError::Unsupported(format!(
                            "protobuf decode failed: {}; json fallback failed: {}",
                            proto_err, json_err
                        ))
                    }),
                }
            }
        }
    }
}
