# DraftChangedEvent

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**event** | **Event** |  (enum: draft.updated, draft.review.updated, draft.comment.created, draft.comment.updated) |
**event_id** | Option<**uuid::Uuid**> |  | [optional]
**occurred_at** | Option<**chrono::DateTime<chrono::FixedOffset>**> |  | [optional]
**payload** | [**models::DraftChangedPayload**](DraftChangedPayload.md) |  |

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)
