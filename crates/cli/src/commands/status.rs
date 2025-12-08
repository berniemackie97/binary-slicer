use anyhow::{anyhow, Result};
use ritual_core::db::RitualRunStatus;

pub fn validate_run_status(status: &str) -> Result<RitualRunStatus> {
    match status {
        "pending" => Ok(RitualRunStatus::Pending),
        "running" => Ok(RitualRunStatus::Running),
        "succeeded" => Ok(RitualRunStatus::Succeeded),
        "failed" => Ok(RitualRunStatus::Failed),
        "canceled" => Ok(RitualRunStatus::Canceled),
        "stubbed" => Ok(RitualRunStatus::Stubbed),
        other => Err(anyhow!(
            "Invalid status '{}'. Allowed: pending, running, succeeded, failed, canceled, stubbed",
            other
        )),
    }
}
