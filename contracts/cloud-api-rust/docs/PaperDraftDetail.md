# PaperDraftDetail

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**access_role** | [**models::DraftAccessRole**](DraftAccessRole.md) |  |
**collaborator_count** | **i32** |  |
**collaborators** | Option<[**Vec<models::PaperDraftCollaboratorEntity>**](PaperDraftCollaboratorEntity.md)> |  | [optional]
**comment_count** | **i32** |  |
**comments** | Option<[**Vec<models::PaperDraftCommentEntity>**](PaperDraftCommentEntity.md)> |  | [optional]
**created_at** | **chrono::DateTime<chrono::FixedOffset>** |  |
**id** | **i32** |  |
**name** | **String** |  |
**open_comment_count** | **i32** |  |
**owner** | Option<[**models::DraftUserRef**](DraftUserRef.md)> |  | [optional]
**public_id** | **String** |  |
**review_status** | [**models::DraftReviewStatus**](DraftReviewStatus.md) |  |
**revision** | **i32** |  |
**state** | **std::collections::HashMap<String, serde_json::Value>** |  |
**updated_at** | **chrono::DateTime<chrono::FixedOffset>** |  |
**updated_by** | Option<[**models::DraftUserRef**](DraftUserRef.md)> |  | [optional]

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)
