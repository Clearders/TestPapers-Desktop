# \QuestionsApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**create_question**](QuestionsApi.md#create_question) | **POST** /api/v1/questions | Create Question
[**create_question_correction**](QuestionsApi.md#create_question_correction) | **POST** /api/v1/questions/{question_public_id}/corrections | Create Question Correction
[**delete_question**](QuestionsApi.md#delete_question) | **DELETE** /api/v1/questions/{question_public_id} | Delete Question
[**delete_question_correction**](QuestionsApi.md#delete_question_correction) | **DELETE** /api/v1/questions/{question_public_id}/corrections/{correction_id} | Delete Question Correction
[**delete_question_revision**](QuestionsApi.md#delete_question_revision) | **DELETE** /api/v1/questions/{question_public_id}/revisions/{revision_id} | Delete Question Revision
[**get_question**](QuestionsApi.md#get_question) | **GET** /api/v1/questions/{question_public_id} | Get Question
[**get_question_corrections**](QuestionsApi.md#get_question_corrections) | **GET** /api/v1/questions/{question_public_id}/corrections | Get Question Corrections
[**get_question_revisions**](QuestionsApi.md#get_question_revisions) | **GET** /api/v1/questions/{question_public_id}/revisions | Get Question Revisions
[**list_my_questions**](QuestionsApi.md#list_my_questions) | **GET** /api/v1/questions/mine | List My Questions
[**list_questions**](QuestionsApi.md#list_questions) | **GET** /api/v1/questions | List Questions
[**update_question**](QuestionsApi.md#update_question) | **PATCH** /api/v1/questions/{question_public_id} | Update Question
[**update_question_correction**](QuestionsApi.md#update_question_correction) | **PATCH** /api/v1/questions/{question_public_id}/corrections/{correction_id} | Update Question Correction



## create_question

> models::EnvelopeQuestionEntity create_question(question_create)
Create Question

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**question_create** | [**QuestionCreate**](QuestionCreate.md) |  | [required] |

### Return type

[**models::EnvelopeQuestionEntity**](Envelope_QuestionEntity_.md)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## create_question_correction

> models::EnvelopeQuestionCorrectionEntity create_question_correction(question_public_id, question_correction_create)
Create Question Correction

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**question_public_id** | **String** |  | [required] |
**question_correction_create** | [**QuestionCorrectionCreate**](QuestionCorrectionCreate.md) |  | [required] |

### Return type

[**models::EnvelopeQuestionCorrectionEntity**](Envelope_QuestionCorrectionEntity_.md)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## delete_question

> delete_question(question_public_id)
Delete Question

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**question_public_id** | **String** |  | [required] |

### Return type

 (empty response body)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## delete_question_correction

> delete_question_correction(question_public_id, correction_id)
Delete Question Correction

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**question_public_id** | **String** |  | [required] |
**correction_id** | **i32** |  | [required] |

### Return type

 (empty response body)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## delete_question_revision

> delete_question_revision(question_public_id, revision_id)
Delete Question Revision

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**question_public_id** | **String** |  | [required] |
**revision_id** | **i32** |  | [required] |

### Return type

 (empty response body)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## get_question

> models::EnvelopeQuestionEntity get_question(question_public_id, include_answer)
Get Question

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**question_public_id** | **String** |  | [required] |
**include_answer** | Option<**bool**> |  |  |[default to true]

### Return type

[**models::EnvelopeQuestionEntity**](Envelope_QuestionEntity_.md)

### Authorization

[cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## get_question_corrections

> models::EnvelopeListQuestionCorrectionEntity get_question_corrections(question_public_id)
Get Question Corrections

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**question_public_id** | **String** |  | [required] |

### Return type

[**models::EnvelopeListQuestionCorrectionEntity**](Envelope_list_QuestionCorrectionEntity__.md)

### Authorization

[cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## get_question_revisions

> models::EnvelopeListQuestionRevisionEntity get_question_revisions(question_public_id)
Get Question Revisions

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**question_public_id** | **String** |  | [required] |

### Return type

[**models::EnvelopeListQuestionRevisionEntity**](Envelope_list_QuestionRevisionEntity__.md)

### Authorization

[cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## list_my_questions

> models::EnvelopePaginatedResponseQuestionEntity list_my_questions(q, subjects, difficulty, r#type, tags, has_latex, include_answer, page, page_size, sort_by, sort_order)
List My Questions

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**q** | Option<**String**> |  |  |
**subjects** | Option<**String**> |  |  |
**difficulty** | Option<[**models::Difficulty**](Models__Difficulty.md)> |  |  |
**r#type** | Option<[**models::QuestionType**](Models__QuestionType.md)> |  |  |
**tags** | Option<**String**> |  |  |
**has_latex** | Option<**bool**> |  |  |
**include_answer** | Option<**bool**> |  |  |[default to true]
**page** | Option<**i32**> |  |  |[default to 1]
**page_size** | Option<**i32**> |  |  |[default to 20]
**sort_by** | Option<**String**> |  |  |
**sort_order** | Option<[**models::SortOrder**](Models__SortOrder.md)> |  |  |[default to desc]

### Return type

[**models::EnvelopePaginatedResponseQuestionEntity**](Envelope_PaginatedResponse_QuestionEntity__.md)

### Authorization

[cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## list_questions

> models::EnvelopePaginatedResponseQuestionEntity list_questions(q, subjects, difficulty, r#type, tags, has_latex, owner_id, include_answer, page, page_size, sort_by, sort_order)
List Questions

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**q** | Option<**String**> |  |  |
**subjects** | Option<**String**> |  |  |
**difficulty** | Option<[**models::Difficulty**](Models__Difficulty.md)> |  |  |
**r#type** | Option<[**models::QuestionType**](Models__QuestionType.md)> |  |  |
**tags** | Option<**String**> |  |  |
**has_latex** | Option<**bool**> |  |  |
**owner_id** | Option<**i32**> |  |  |
**include_answer** | Option<**bool**> |  |  |[default to true]
**page** | Option<**i32**> |  |  |[default to 1]
**page_size** | Option<**i32**> |  |  |[default to 20]
**sort_by** | Option<**String**> |  |  |
**sort_order** | Option<[**models::SortOrder**](Models__SortOrder.md)> |  |  |[default to desc]

### Return type

[**models::EnvelopePaginatedResponseQuestionEntity**](Envelope_PaginatedResponse_QuestionEntity__.md)

### Authorization

[cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## update_question

> models::EnvelopeQuestionEntity update_question(question_public_id, question_update)
Update Question

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**question_public_id** | **String** |  | [required] |
**question_update** | [**QuestionUpdate**](QuestionUpdate.md) |  | [required] |

### Return type

[**models::EnvelopeQuestionEntity**](Envelope_QuestionEntity_.md)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## update_question_correction

> models::EnvelopeQuestionCorrectionEntity update_question_correction(question_public_id, correction_id, question_correction_update)
Update Question Correction

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**question_public_id** | **String** |  | [required] |
**correction_id** | **i32** |  | [required] |
**question_correction_update** | [**QuestionCorrectionUpdate**](QuestionCorrectionUpdate.md) |  | [required] |

### Return type

[**models::EnvelopeQuestionCorrectionEntity**](Envelope_QuestionCorrectionEntity_.md)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)
