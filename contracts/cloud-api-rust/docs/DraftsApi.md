# \DraftsApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**create_comment**](DraftsApi.md#create_comment) | **POST** /api/v1/drafts/{draft_public_id}/comments | Create Comment
[**create_draft**](DraftsApi.md#create_draft) | **POST** /api/v1/drafts | Create Draft
[**create_or_update_collaborator**](DraftsApi.md#create_or_update_collaborator) | **POST** /api/v1/drafts/{draft_public_id}/collaborators | Create Or Update Collaborator
[**delete_draft**](DraftsApi.md#delete_draft) | **DELETE** /api/v1/drafts/{draft_public_id} | Delete Draft
[**download_draft**](DraftsApi.md#download_draft) | **GET** /api/v1/drafts/{draft_public_id}/download | Download Draft
[**get_draft**](DraftsApi.md#get_draft) | **GET** /api/v1/drafts/{draft_public_id} | Get Draft
[**list_drafts**](DraftsApi.md#list_drafts) | **GET** /api/v1/drafts | List Drafts
[**patch_collaborator**](DraftsApi.md#patch_collaborator) | **PATCH** /api/v1/drafts/{draft_public_id}/collaborators/{user_public_id} | Patch Collaborator
[**patch_comment**](DraftsApi.md#patch_comment) | **PATCH** /api/v1/drafts/{draft_public_id}/comments/{comment_public_id} | Patch Comment
[**remove_collaborator**](DraftsApi.md#remove_collaborator) | **DELETE** /api/v1/drafts/{draft_public_id}/collaborators/{user_public_id} | Remove Collaborator
[**update_draft**](DraftsApi.md#update_draft) | **PATCH** /api/v1/drafts/{draft_public_id} | Update Draft



## create_comment

> models::EnvelopePaperDraftDetail create_comment(draft_public_id, paper_draft_comment_create)
Create Comment

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**draft_public_id** | **String** |  | [required] |
**paper_draft_comment_create** | [**PaperDraftCommentCreate**](PaperDraftCommentCreate.md) |  | [required] |

### Return type

[**models::EnvelopePaperDraftDetail**](Envelope_PaperDraftDetail_.md)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## create_draft

> models::EnvelopePaperDraftDetail create_draft(paper_draft_create)
Create Draft

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**paper_draft_create** | [**PaperDraftCreate**](PaperDraftCreate.md) |  | [required] |

### Return type

[**models::EnvelopePaperDraftDetail**](Envelope_PaperDraftDetail_.md)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## create_or_update_collaborator

> models::EnvelopePaperDraftDetail create_or_update_collaborator(draft_public_id, paper_draft_collaborator_create)
Create Or Update Collaborator

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**draft_public_id** | **String** |  | [required] |
**paper_draft_collaborator_create** | [**PaperDraftCollaboratorCreate**](PaperDraftCollaboratorCreate.md) |  | [required] |

### Return type

[**models::EnvelopePaperDraftDetail**](Envelope_PaperDraftDetail_.md)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## delete_draft

> delete_draft(draft_public_id)
Delete Draft

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**draft_public_id** | **String** |  | [required] |

### Return type

 (empty response body)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## download_draft

> std::path::PathBuf download_draft(draft_public_id)
Download Draft

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**draft_public_id** | **String** |  | [required] |

### Return type

[**std::path::PathBuf**](std::path::PathBuf.md)

### Authorization

[cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/vnd.openxmlformats-officedocument.wordprocessingml.document, application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## get_draft

> models::EnvelopePaperDraftDetail get_draft(draft_public_id)
Get Draft

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**draft_public_id** | **String** |  | [required] |

### Return type

[**models::EnvelopePaperDraftDetail**](Envelope_PaperDraftDetail_.md)

### Authorization

[cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## list_drafts

> models::EnvelopeListPaperDraftSummary list_drafts()
List Drafts

### Parameters

This endpoint does not need any parameter.

### Return type

[**models::EnvelopeListPaperDraftSummary**](Envelope_list_PaperDraftSummary__.md)

### Authorization

[cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## patch_collaborator

> models::EnvelopePaperDraftDetail patch_collaborator(draft_public_id, user_public_id, paper_draft_collaborator_update)
Patch Collaborator

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**draft_public_id** | **String** |  | [required] |
**user_public_id** | **String** |  | [required] |
**paper_draft_collaborator_update** | [**PaperDraftCollaboratorUpdate**](PaperDraftCollaboratorUpdate.md) |  | [required] |

### Return type

[**models::EnvelopePaperDraftDetail**](Envelope_PaperDraftDetail_.md)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## patch_comment

> models::EnvelopePaperDraftDetail patch_comment(draft_public_id, comment_public_id, paper_draft_comment_update)
Patch Comment

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**draft_public_id** | **String** |  | [required] |
**comment_public_id** | **String** |  | [required] |
**paper_draft_comment_update** | [**PaperDraftCommentUpdate**](PaperDraftCommentUpdate.md) |  | [required] |

### Return type

[**models::EnvelopePaperDraftDetail**](Envelope_PaperDraftDetail_.md)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## remove_collaborator

> models::EnvelopePaperDraftDetail remove_collaborator(draft_public_id, user_public_id)
Remove Collaborator

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**draft_public_id** | **String** |  | [required] |
**user_public_id** | **String** |  | [required] |

### Return type

[**models::EnvelopePaperDraftDetail**](Envelope_PaperDraftDetail_.md)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## update_draft

> models::EnvelopePaperDraftDetail update_draft(draft_public_id, paper_draft_update)
Update Draft

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**draft_public_id** | **String** |  | [required] |
**paper_draft_update** | [**PaperDraftUpdate**](PaperDraftUpdate.md) |  | [required] |

### Return type

[**models::EnvelopePaperDraftDetail**](Envelope_PaperDraftDetail_.md)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)
