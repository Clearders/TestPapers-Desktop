# QuestionChangedEvent

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**event** | **Event** |  (enum: question.created, question.updated) |
**event_id** | Option<**uuid::Uuid**> |  | [optional]
**occurred_at** | Option<**chrono::DateTime<chrono::FixedOffset>**> |  | [optional]
**payload** | [**models::QuestionChangedPayload**](QuestionChangedPayload.md) |  |

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)
