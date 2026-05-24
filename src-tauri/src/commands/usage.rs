use crate::database::*;
use crate::error::AppError;
use crate::services::log_service;
use crate::AppState;
use serde::Deserialize;
use tauri::State;

#[tauri::command]
pub fn get_usage_logs(
    state: State<'_, AppState>,
    filter: UsageLogFilter,
) -> Result<PaginatedResult<UsageLog>, AppError> {
    log_service::get_usage_logs(&state.db, &filter)
}

#[tauri::command]
pub fn get_dashboard_stats(
    state: State<'_, AppState>,
    filter: Option<DashboardFilterParams>,
) -> Result<DashboardStats, AppError> {
    let (start, end, _) = parse_filter(filter);
    log_service::get_dashboard_stats(&state.db, start, end)
}

#[tauri::command]
pub fn get_model_consumption(
    state: State<'_, AppState>,
    filter: Option<DashboardFilterParams>,
) -> Result<Vec<ChartDataPoint>, AppError> {
    let (start, end, granularity) = parse_filter(filter);
    log_service::get_model_consumption(&state.db, start, end, granularity.as_deref())
}

#[tauri::command]
pub fn get_call_trend(
    state: State<'_, AppState>,
    filter: Option<DashboardFilterParams>,
) -> Result<Vec<ChartDataPoint>, AppError> {
    let (start, end, granularity) = parse_filter(filter);
    log_service::get_call_trend(&state.db, start, end, granularity.as_deref())
}

#[tauri::command]
pub fn get_model_distribution(
    state: State<'_, AppState>,
    filter: Option<DashboardFilterParams>,
) -> Result<Vec<ModelRanking>, AppError> {
    let (start, end, _) = parse_filter(filter);
    log_service::get_model_distribution(&state.db, start, end)
}

#[tauri::command]
pub fn get_model_ranking(
    state: State<'_, AppState>,
    filter: Option<DashboardFilterParams>,
) -> Result<Vec<ModelRanking>, AppError> {
    let (start, end, _) = parse_filter(filter);
    log_service::get_model_ranking(&state.db, start, end)
}

#[tauri::command]
pub fn get_user_ranking(
    state: State<'_, AppState>,
    filter: Option<DashboardFilterParams>,
) -> Result<Vec<UserRanking>, AppError> {
    let (start, end, _) = parse_filter(filter);
    log_service::get_user_ranking(&state.db, start, end)
}

#[tauri::command]
pub fn get_user_trend(
    state: State<'_, AppState>,
    filter: Option<DashboardFilterParams>,
) -> Result<Vec<ChartDataPoint>, AppError> {
    let (start, end, granularity) = parse_filter(filter);
    log_service::get_user_trend(&state.db, start, end, granularity.as_deref())
}

#[derive(Deserialize)]
pub struct DashboardFilterParams {
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
    pub granularity: Option<String>,
}

fn parse_filter(
    filter: Option<DashboardFilterParams>,
) -> (Option<i64>, Option<i64>, Option<String>) {
    match filter {
        Some(f) => (f.start_time, f.end_time, f.granularity),
        None => (None, None, None),
    }
}
