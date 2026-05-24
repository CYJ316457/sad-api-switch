use crate::admin::error::AdminError;
use crate::admin::state::AdminState;
use crate::database::{
    ChartDataPoint, DashboardStats, ModelRanking, UsageLog, UsageLogFilter, UserRanking,
};
use crate::services::log_service;
use axum::{
    extract::{Query, State},
    Json,
};
use serde::Deserialize;

// Query parameter structs
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct LogsQueryParams {
    pub page: Option<i32>,
    pub page_size: Option<i32>,
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
    pub model: Option<String>,
    pub channel_id: Option<String>,
    pub access_key_id: Option<String>,
    pub request_id: Option<String>,
    pub success: Option<bool>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct DashboardQueryParams {
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
    pub granularity: Option<String>,
}

// Helper to convert query params to UsageLogFilter
fn query_to_usage_log_filter(params: &LogsQueryParams) -> UsageLogFilter {
    UsageLogFilter {
        page: params.page,
        page_size: params.page_size,
        start_time: params.start_time,
        end_time: params.end_time,
        model: params.model.clone(),
        channel_id: params.channel_id.clone(),
        access_key_id: params.access_key_id.clone(),
        request_id: params.request_id.clone(),
        success: params.success,
    }
}

// Helper to convert query params to DashboardFilterParams
fn query_to_dashboard_filter(
    params: &DashboardQueryParams,
) -> Option<crate::commands::usage::DashboardFilterParams> {
    if params.start_time.is_none() && params.end_time.is_none() && params.granularity.is_none() {
        None
    } else {
        Some(crate::commands::usage::DashboardFilterParams {
            start_time: params.start_time,
            end_time: params.end_time,
            granularity: params.granularity.clone(),
        })
    }
}

// ---------- Handlers -------------------------------------------------------

pub async fn get_logs(
    State(state): State<AdminState>,
    Query(params): Query<LogsQueryParams>,
) -> Result<Json<crate::database::PaginatedResult<UsageLog>>, AdminError> {
    let filter = query_to_usage_log_filter(&params);
    let result = log_service::get_usage_logs(&state.db, &filter)?;
    Ok(Json(result))
}

pub async fn get_dashboard_stats(
    State(state): State<AdminState>,
    Query(params): Query<DashboardQueryParams>,
) -> Result<Json<DashboardStats>, AdminError> {
    let filter = query_to_dashboard_filter(&params);
    let (start, end, _) = if let Some(f) = filter {
        (f.start_time, f.end_time, f.granularity)
    } else {
        (None, None, None)
    };
    let result = log_service::get_dashboard_stats(&state.db, start, end)?;
    Ok(Json(result))
}

pub async fn get_model_consumption(
    State(state): State<AdminState>,
    Query(params): Query<DashboardQueryParams>,
) -> Result<Json<Vec<ChartDataPoint>>, AdminError> {
    let filter = query_to_dashboard_filter(&params);
    let (start, end, granularity) = if let Some(f) = filter {
        (f.start_time, f.end_time, f.granularity)
    } else {
        (None, None, None)
    };
    let result = log_service::get_model_consumption(&state.db, start, end, granularity.as_deref())?;
    Ok(Json(result))
}

pub async fn get_call_trend(
    State(state): State<AdminState>,
    Query(params): Query<DashboardQueryParams>,
) -> Result<Json<Vec<ChartDataPoint>>, AdminError> {
    let filter = query_to_dashboard_filter(&params);
    let (start, end, granularity) = if let Some(f) = filter {
        (f.start_time, f.end_time, f.granularity)
    } else {
        (None, None, None)
    };
    let result = log_service::get_call_trend(&state.db, start, end, granularity.as_deref())?;
    Ok(Json(result))
}

pub async fn get_model_distribution(
    State(state): State<AdminState>,
    Query(params): Query<DashboardQueryParams>,
) -> Result<Json<Vec<ModelRanking>>, AdminError> {
    let filter = query_to_dashboard_filter(&params);
    let (start, end, _) = if let Some(f) = filter {
        (f.start_time, f.end_time, f.granularity)
    } else {
        (None, None, None)
    };
    let result = log_service::get_model_distribution(&state.db, start, end)?;
    Ok(Json(result))
}

pub async fn get_model_ranking(
    State(state): State<AdminState>,
    Query(params): Query<DashboardQueryParams>,
) -> Result<Json<Vec<ModelRanking>>, AdminError> {
    let filter = query_to_dashboard_filter(&params);
    let (start, end, _) = if let Some(f) = filter {
        (f.start_time, f.end_time, f.granularity)
    } else {
        (None, None, None)
    };
    let result = log_service::get_model_ranking(&state.db, start, end)?;
    Ok(Json(result))
}

pub async fn get_user_ranking(
    State(state): State<AdminState>,
    Query(params): Query<DashboardQueryParams>,
) -> Result<Json<Vec<UserRanking>>, AdminError> {
    let filter = query_to_dashboard_filter(&params);
    let (start, end, _) = if let Some(f) = filter {
        (f.start_time, f.end_time, f.granularity)
    } else {
        (None, None, None)
    };
    let result = log_service::get_user_ranking(&state.db, start, end)?;
    Ok(Json(result))
}

pub async fn get_user_trend(
    State(state): State<AdminState>,
    Query(params): Query<DashboardQueryParams>,
) -> Result<Json<Vec<ChartDataPoint>>, AdminError> {
    let filter = query_to_dashboard_filter(&params);
    let (start, end, granularity) = if let Some(f) = filter {
        (f.start_time, f.end_time, f.granularity)
    } else {
        (None, None, None)
    };
    let result = log_service::get_user_trend(&state.db, start, end, granularity.as_deref())?;
    Ok(Json(result))
}
