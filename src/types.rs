//! Data types for EC2 Monitor

use crate::error::MonitorError;
use std::fmt::Display;

#[derive(Debug, Default, Clone)]
#[allow(dead_code)]
pub struct InstanceInfo {
    pub instance_id: String,
    pub name: String,
    pub instance_type: String,
    pub public_ip: Option<String>,
    pub private_ip: Option<String>,
}

#[derive(Debug, Default)]
pub struct InstanceResults {
    pub instance_id: String,
    pub public_ip: Option<String>,
    pub name: String,
    pub instance_type: String,
    pub timestep_result: Option<TimeStep>,
    pub csv_count: Option<i32>,
    pub free_disk_space: Option<String>,
    pub current_process: Option<String>,
    pub eta: Option<String>,
    pub connection_error: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub struct TimeStep {
    pub step: usize,
    pub time: f64,
    pub total_step: usize,
    pub step_increase: Option<usize>,
}

impl TimeStep {
    pub fn new(case: &str, time_step: &str) -> Result<Self, MonitorError> {
        let Some(i) = time_step.find(':') else {
            return Ok(Default::default());
        };
        let (a, b) = time_step.split_at(i);
        let steps = case.split('_').find_map(|x| match x {
            "2ms" => Some(24_000),
            "7ms" | "12ms" | "17ms" => Some(18_000),
            _ => None,
        });
        Ok(Self {
            step: a[8..].trim().parse::<usize>()?,
            time: b[6..].trim().parse::<f64>()?,
            total_step: steps.ok_or(MonitorError::InvalidWindSpeed)?,
            step_increase: None,
        })
    }

    pub fn calculate_eta(&self) -> Option<String> {
        // Only calculate ETA if we have a step increase (not the first run)
        if let Some(step_increase) = self.step_increase {
            if step_increase > 0 {
                let remaining_steps = self.total_step.saturating_sub(self.step);
                if remaining_steps == 0 {
                    return Some("Complete".to_string());
                }

                // Calculate minutes needed based on current rate
                // step_increase is over 6 minutes, so minutes per step = 6 / step_increase
                let minutes_per_step = 6.0 / step_increase as f64;
                let total_minutes = remaining_steps as f64 * minutes_per_step;

                // Convert to days, hours and minutes
                let total_hours = total_minutes / 60.0;
                let days = (total_hours / 24.0).floor() as u64;
                let hours = (total_hours % 24.0).floor() as u64;
                let minutes = (total_minutes % 60.0).round() as u64;

                if days > 0 {
                    if hours > 0 {
                        return Some(format!("{}d {}h {}m", days, hours, minutes));
                    } else {
                        return Some(format!("{}d {}m", days, minutes));
                    }
                } else if hours > 0 {
                    return Some(format!("{}h {}m", hours, minutes));
                } else {
                    return Some(format!("{}m", minutes));
                }
            } else {
                return Some("Stalled".to_string());
            }
        }

        None // First run, no ETA available
    }
}

impl Display for TimeStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(inc) = self.step_increase {
            write!(f, "(+{:}){:8.2}", inc, self.time)
        } else {
            write!(f, "({}){:8.2}", self.step, self.time)
        }
    }
}
