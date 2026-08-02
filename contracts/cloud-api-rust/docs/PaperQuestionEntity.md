# PaperQuestionEntity

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**answer** | Option<[**models::Answer**](Answer.md)> |  | [optional]
**created_at** | **chrono::DateTime<chrono::FixedOffset>** |  |
**difficulty** | [**models::Difficulty**](Difficulty.md) |  |
**essay_blank_space** | Option<[**models::EssayBlankSpace**](EssayBlankSpace.md)> |  | [optional]
**has_latex** | Option<**bool**> |  | [optional]
**id** | **i32** |  |
**images** | Option<[**Vec<models::QuestionImage>**](QuestionImage.md)> |  | [optional]
**marks** | Option<**i32**> |  | [optional]
**options** | Option<**Vec<String>**> |  | [optional]
**order_no** | **i32** |  |
**owner_id** | Option<**i32**> |  | [optional]
**public_id** | **String** |  |
**question_public_id** | **String** |  |
**score_weight** | Option<**f64**> |  | [optional][default to 1.0]
**source** | Option<**String**> |  | [optional]
**subjects** | **Vec<String>** |  |
**tags** | Option<**Vec<String>**> |  | [optional]
**text** | **String** |  |
**r#type** | [**models::QuestionType**](QuestionType.md) |  |
**updated_at** | **chrono::DateTime<chrono::FixedOffset>** |  |

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)
