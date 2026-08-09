# \PapersApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**add_paper_questions**](PapersApi.md#add_paper_questions) | **POST** /api/v1/papers/{paper_public_id}/questions | Add Paper Questions
[**create_paper**](PapersApi.md#create_paper) | **POST** /api/v1/papers | Create Paper
[**download_draft_paper**](PapersApi.md#download_draft_paper) | **POST** /api/v1/papers/draft-download | Download Draft Paper
[**download_paper**](PapersApi.md#download_paper) | **GET** /api/v1/papers/{paper_public_id}/download | Download Paper
[**export_preview**](PapersApi.md#export_preview) | **POST** /api/v1/papers/{paper_public_id}/export-preview | Export Preview
[**generate_paper**](PapersApi.md#generate_paper) | **POST** /api/v1/papers/generate | Generate Paper
[**get_paper**](PapersApi.md#get_paper) | **GET** /api/v1/papers/{paper_public_id} | Get Paper
[**remove_paper_question**](PapersApi.md#remove_paper_question) | **DELETE** /api/v1/papers/{paper_public_id}/questions/{question_public_id} | Remove Paper Question
[**reorder_paper_questions**](PapersApi.md#reorder_paper_questions) | **PUT** /api/v1/papers/{paper_public_id}/questions/order | Reorder Paper Questions
[**replace_paper_questions**](PapersApi.md#replace_paper_questions) | **PUT** /api/v1/papers/{paper_public_id}/questions | Replace Paper Questions
[**update_paper**](PapersApi.md#update_paper) | **PATCH** /api/v1/papers/{paper_public_id} | Update Paper



## add_paper_questions

> models::EnvelopePaperExpandedEntity add_paper_questions(paper_public_id, question_ref)
Add Paper Questions

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**paper_public_id** | **String** |  | [required] |
**question_ref** | [**Vec<models::QuestionRef>**](QuestionRef.md) |  | [required] |

### Return type

[**models::EnvelopePaperExpandedEntity**](Envelope_PaperExpandedEntity_.md)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## create_paper

> models::EnvelopePaperExpandedEntity create_paper(paper_create)
Create Paper

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**paper_create** | [**PaperCreate**](PaperCreate.md) |  | [required] |

### Return type

[**models::EnvelopePaperExpandedEntity**](Envelope_PaperExpandedEntity_.md)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## download_draft_paper

> std::path::PathBuf download_draft_paper(paper_draft_download_request, format)
Download Draft Paper

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**paper_draft_download_request** | [**PaperDraftDownloadRequest**](PaperDraftDownloadRequest.md) |  | [required] |
**format** | Option<**String**> |  |  |[default to docx]

### Return type

[**std::path::PathBuf**](std::path::PathBuf.md)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/vnd.openxmlformats-officedocument.wordprocessingml.document, application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## download_paper

> std::path::PathBuf download_paper(paper_public_id, format, question_order, include_answer, layout_density)
Download Paper

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**paper_public_id** | **String** |  | [required] |
**format** | Option<**String**> |  |  |[default to docx]
**question_order** | Option<**String**> |  |  |[default to paper]
**include_answer** | Option<**bool**> |  |  |[default to true]
**layout_density** | Option<**String**> |  |  |[default to auto]

### Return type

[**std::path::PathBuf**](std::path::PathBuf.md)

### Authorization

[cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/vnd.openxmlformats-officedocument.wordprocessingml.document, application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## export_preview

> models::EnvelopeDict export_preview(paper_public_id, export_preview_request)
Export Preview

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**paper_public_id** | **String** |  | [required] |
**export_preview_request** | [**ExportPreviewRequest**](ExportPreviewRequest.md) |  | [required] |

### Return type

[**models::EnvelopeDict**](Envelope_dict_.md)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## generate_paper

> serde_json::Value generate_paper(paper_generate_request)
Generate Paper

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**paper_generate_request** | [**PaperGenerateRequest**](PaperGenerateRequest.md) |  | [required] |

### Return type

[**serde_json::Value**](serde_json::Value.md)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## get_paper

> models::EnvelopeUnionPaperExpandedEntityPaperEntity get_paper(paper_public_id, expand, include_answer)
Get Paper

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**paper_public_id** | **String** |  | [required] |
**expand** | Option<**String**> |  |  |
**include_answer** | Option<**bool**> |  |  |[default to true]

### Return type

[**models::EnvelopeUnionPaperExpandedEntityPaperEntity**](Envelope_Union_PaperExpandedEntity__PaperEntity__.md)

### Authorization

[cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## remove_paper_question

> models::EnvelopePaperExpandedEntity remove_paper_question(paper_public_id, question_public_id)
Remove Paper Question

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**paper_public_id** | **String** |  | [required] |
**question_public_id** | **String** |  | [required] |

### Return type

[**models::EnvelopePaperExpandedEntity**](Envelope_PaperExpandedEntity_.md)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## reorder_paper_questions

> models::EnvelopePaperExpandedEntity reorder_paper_questions(paper_public_id, question_order_update)
Reorder Paper Questions

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**paper_public_id** | **String** |  | [required] |
**question_order_update** | [**QuestionOrderUpdate**](QuestionOrderUpdate.md) |  | [required] |

### Return type

[**models::EnvelopePaperExpandedEntity**](Envelope_PaperExpandedEntity_.md)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## replace_paper_questions

> models::EnvelopePaperExpandedEntity replace_paper_questions(paper_public_id, question_ref)
Replace Paper Questions

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**paper_public_id** | **String** |  | [required] |
**question_ref** | [**Vec<models::QuestionRef>**](QuestionRef.md) |  | [required] |

### Return type

[**models::EnvelopePaperExpandedEntity**](Envelope_PaperExpandedEntity_.md)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## update_paper

> models::EnvelopePaperEntity update_paper(paper_public_id, paper_update)
Update Paper

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**paper_public_id** | **String** |  | [required] |
**paper_update** | [**PaperUpdate**](PaperUpdate.md) |  | [required] |

### Return type

[**models::EnvelopePaperEntity**](Envelope_PaperEntity_.md)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)
