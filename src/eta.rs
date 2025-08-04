//! ETA parsing and median calculation utilities

/// Parse ETA string back to total minutes for calculation
pub fn parse_eta_to_minutes(eta_str: &str) -> Option<f64> {
    // Skip special cases
    if eta_str == "Complete" || eta_str == "Stalled" || eta_str == "Calculating..." {
        return None;
    }

    let mut total_minutes = 0.0;

    // Parse days, hours, minutes format like "2d 5h 30m" or "45m" or "3h 15m"
    for part in eta_str.split_whitespace() {
        if let Some(stripped) = part.strip_suffix('d') {
            if let Ok(days) = stripped.parse::<f64>() {
                total_minutes += days * 24.0 * 60.0;
            }
        } else if let Some(stripped) = part.strip_suffix('h') {
            if let Ok(hours) = stripped.parse::<f64>() {
                total_minutes += hours * 60.0;
            }
        } else if let Some(stripped) = part.strip_suffix('m') {
            if let Ok(minutes) = stripped.parse::<f64>() {
                total_minutes += minutes;
            }
        }
    }

    if total_minutes > 0.0 {
        Some(total_minutes)
    } else {
        None
    }
}

/// Calculate median ETA from a collection of ETA values in minutes
pub fn calculate_median_eta(etas: &[f64]) -> Option<String> {
    if etas.is_empty() {
        return None;
    }

    let mut sorted_etas = etas.to_vec();
    sorted_etas.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let median_minutes = if sorted_etas.len() % 2 == 0 {
        // Even number of elements - average of two middle values
        let mid = sorted_etas.len() / 2;
        (sorted_etas[mid - 1] + sorted_etas[mid]) / 2.0
    } else {
        // Odd number of elements - middle value
        sorted_etas[sorted_etas.len() / 2]
    };

    // Convert median minutes to days, hours, minutes format
    let total_hours = median_minutes / 60.0;
    let days = (total_hours / 24.0).floor() as u64;
    let hours = (total_hours % 24.0).floor() as u64;
    let minutes = (median_minutes % 60.0).round() as u64;

    if days > 0 {
        if hours > 0 {
            Some(format!("{}d {}h {}m", days, hours, minutes))
        } else {
            Some(format!("{}d {}m", days, minutes))
        }
    } else if hours > 0 {
        Some(format!("{}h {}m", hours, minutes))
    } else {
        Some(format!("{}m", minutes))
    }
}