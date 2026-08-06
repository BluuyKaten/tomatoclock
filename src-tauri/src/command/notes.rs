//! 学习记录命令（对齐上游 §7.5）

use tauri::{State};

use crate::command::helpers::ok;
use crate::domain::requests::*;
use crate::domain::responses::*;
use crate::error::AppError;
use crate::repository::study_notes::StudyNoteRepo;
use crate::AppState;

/// POST /api/notes
#[tauri::command]
pub fn create_note(
    state: State<'_, AppState>,
    req: CreateNoteRequest,
) -> Result<ApiResponse<CreateNoteResponse>, AppError> {
    let user_id = state.current_user_id()?;
    let pool = state.db();
    if req.content.is_empty() {
        return Err(AppError::InvalidParam("笔记内容不能为空".into()));
    }
    let tags_json = req.tags.as_ref().map(|t| serde_json::to_string(t).ok()).flatten();
    let images = req.image_paths.clone().unwrap_or_default();
    let id = StudyNoteRepo::create(
        pool,
        user_id,
        req.pomodoro_id,
        req.subject_id,
        req.title.as_deref(),
        &req.content,
        tags_json.as_deref(),
        &images,
    )?;
    // 若关联番茄，回填 pomodoros.note_id（解决循环外键反查）
    if let Some(pomo_id) = req.pomodoro_id {
        crate::repository::pomodoros::PomodoroRepo::set_note_id(pool, pomo_id, id)?;
    }
    ok(CreateNoteResponse {
        id,
        created_at: crate::repository::now_ms(),
    })
}

/// GET /api/notes
#[tauri::command]
pub fn list_notes(
    state: State<'_, AppState>,
    req: ListNotesRequest,
) -> Result<ApiResponse<NoteListResponse>, AppError> {
    let user_id = state.current_user_id()?;
    let pool = state.db();
    let page = req.page.unwrap_or(1).max(1);
    let page_size = req.page_size.unwrap_or(20).clamp(1, 100);
    let (total, items) = StudyNoteRepo::list(
        pool,
        user_id,
        req.subject_id,
        req.tag.as_deref(),
        req.from,
        req.to,
        page,
        page_size,
    )?;
    let views: Vec<NoteView> = items
        .into_iter()
        .map(|(note, images)| NoteView {
            id: note.id,
            user_id: note.user_id,
            pomodoro_id: note.pomodoro_id,
            subject_id: note.subject_id,
            title: note.title,
            content: note.content,
            tags: note.tags.and_then(|t| serde_json::from_str::<Vec<String>>(&t).ok()),
            images: images
                .into_iter()
                .map(|img| NoteImageView {
                    id: img.id,
                    file_path: img.file_path,
                    mime_type: img.mime_type,
                    size_bytes: img.size_bytes,
                })
                .collect(),
            created_at: note.created_at,
            updated_at: note.updated_at,
        })
        .collect();
    ok(NoteListResponse { total, items: views })
}

/// PUT /api/notes/{id}
#[tauri::command]
pub fn update_note(
    state: State<'_, AppState>,
    id: i64,
    req: UpdateNoteRequest,
) -> Result<ApiResponse<UpdateNoteResponse>, AppError> {
    let user_id = state.current_user_id()?;
    let pool = state.db();
    let (note, _) = StudyNoteRepo::find_by_id(pool, id)?
        .ok_or_else(|| AppError::NotFound("笔记不存在".into()))?;
    if note.user_id != user_id {
        return Err(AppError::AuthError("无权操作".into()));
    }
    let tags_json = req.tags.as_ref().map(|t| serde_json::to_string(t).ok()).flatten();
    StudyNoteRepo::update(
        pool,
        id,
        req.title.as_deref(),
        req.content.as_deref(),
        tags_json.as_deref(),
    )?;
    ok(UpdateNoteResponse {
        id,
        updated_at: crate::repository::now_ms(),
    })
}

/// DELETE /api/notes/{id}
#[tauri::command]
pub fn delete_note(
    state: State<'_, AppState>,
    id: i64,
) -> Result<ApiResponse<serde_json::Value>, AppError> {
    let user_id = state.current_user_id()?;
    let pool = state.db();
    let (note, _) = StudyNoteRepo::find_by_id(pool, id)?
        .ok_or_else(|| AppError::NotFound("笔记不存在".into()))?;
    if note.user_id != user_id {
        return Err(AppError::AuthError("无权操作".into()));
    }
    StudyNoteRepo::delete(pool, id)?;
    ok(serde_json::json!({ "id": id }))
}
