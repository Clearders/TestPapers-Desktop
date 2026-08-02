# PaperDraftQuestion

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**answer** | Option<[**models::Answer**](Answer.md)> |  | [optional]
**difficulty** | Option<[**models::Difficulty**](Difficulty.md)> |  | [optional][default to Medium]
**essay_blank_space** | Option<[**models::EssayBlankSpace**](EssayBlankSpace.md)> |  | [optional]
**has_latex** | Option<**bool**> |  | [optional][default to false]
**images** | Option<[**Vec<models::QuestionImage>**](QuestionImage.md)> |  | [optional]
**marks** | Option<**i32**> |  | [optional]
**options** | Option<**Vec<String>**> |  | [optional]
**order_no** | **i32** |  |
**question_public_id** | **String** |  |
**score_weight** | Option<**f64**> |  | [optional][default to 1]
**source** | Option<**String**> |  | [optional]
**subjects** | Option<**Vec<String>**> |  | [optional]
**tags** | Option<**Vec<String>**> |  | [optional]
**text** | **String** |  |
**r#type** | [**models::QuestionType**](QuestionType.md) |  |

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)
