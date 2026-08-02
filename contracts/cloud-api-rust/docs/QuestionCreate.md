# QuestionCreate

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**answer** | Option<[**models::Answer**](Answer.md)> |  | [optional]
**difficulty** | [**models::Difficulty**](Difficulty.md) |  |
**essay_blank_space** | Option<[**models::EssayBlankSpace**](EssayBlankSpace.md)> |  | [optional]
**has_latex** | Option<**bool**> |  | [optional]
**images** | Option<[**Vec<models::QuestionImage>**](QuestionImage.md)> |  | [optional]
**options** | Option<**Vec<String>**> |  | [optional]
**owner_id** | Option<**i32**> |  | [optional]
**score_weight** | Option<**f64**> |  | [optional][default to 1.0]
**source** | Option<**String**> |  | [optional]
**subjects** | **Vec<String>** |  |
**tags** | Option<**Vec<String>**> |  | [optional]
**text** | **String** |  |
**r#type** | [**models::QuestionType**](QuestionType.md) |  |

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)
