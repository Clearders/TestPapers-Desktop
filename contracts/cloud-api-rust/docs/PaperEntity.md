# PaperEntity

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**created_at** | **chrono::DateTime<chrono::FixedOffset>** |  |
**duration** | **i32** |  |
**id** | **i32** |  |
**owner_id** | Option<**i32**> |  | [optional]
**public_id** | **String** |  |
**questions** | [**Vec<models::QuestionRef>**](QuestionRef.md) |  |
**status** | Option<[**models::PaperStatus**](PaperStatus.md)> |  | [optional][default to Draft]
**subject** | **String** |  |
**title** | **String** |  |
**total_marks** | **i32** |  |
**updated_at** | **chrono::DateTime<chrono::FixedOffset>** |  |

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)
