# \TasksApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**task_cleanup_expired_sessions**](TasksApi.md#task_cleanup_expired_sessions) | **POST** /api/v1/tasks/cleanup-expired-sessions | Task Cleanup Expired Sessions
[**task_compute_question_stats**](TasksApi.md#task_compute_question_stats) | **POST** /api/v1/tasks/stats/questions | Task Compute Question Stats
[**task_export_paper**](TasksApi.md#task_export_paper) | **POST** /api/v1/tasks/export-paper/{paper_public_id} | Task Export Paper
[**task_ping**](TasksApi.md#task_ping) | **POST** /api/v1/tasks/ping | Task Ping
[**task_status**](TasksApi.md#task_status) | **GET** /api/v1/tasks/{task_id} | Task Status
[**task_validate_all_questions**](TasksApi.md#task_validate_all_questions) | **POST** /api/v1/tasks/validate-questions | Task Validate All Questions
[**task_validate_question**](TasksApi.md#task_validate_question) | **POST** /api/v1/tasks/validate-question/{question_public_id} | Task Validate Question



## task_cleanup_expired_sessions

> serde_json::Value task_cleanup_expired_sessions()
Task Cleanup Expired Sessions

Dispatch an async cleanup of expired auth tokens.

### Parameters

This endpoint does not need any parameter.

### Return type

[**serde_json::Value**](serde_json::Value.md)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## task_compute_question_stats

> serde_json::Value task_compute_question_stats()
Task Compute Question Stats

Dispatch async question statistics computation.

### Parameters

This endpoint does not need any parameter.

### Return type

[**serde_json::Value**](serde_json::Value.md)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## task_export_paper

> serde_json::Value task_export_paper(paper_public_id, question_order, include_answer, format)
Task Export Paper

Dispatch an asynchronous paper export. Returns a task ID for polling.

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**paper_public_id** | **String** |  | [required] |
**question_order** | Option<**String**> |  |  |[default to paper]
**include_answer** | Option<**bool**> |  |  |[default to true]
**format** | Option<**String**> |  |  |[default to json]

### Return type

[**serde_json::Value**](serde_json::Value.md)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## task_ping

> serde_json::Value task_ping()
Task Ping

Dispatch a Celery ping task and return its task ID for polling.

### Parameters

This endpoint does not need any parameter.

### Return type

[**serde_json::Value**](serde_json::Value.md)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## task_status

> serde_json::Value task_status(task_id)
Task Status

Poll the status/result of any Celery task by ID.

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**task_id** | **String** |  | [required] |

### Return type

[**serde_json::Value**](serde_json::Value.md)

### Authorization

[cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## task_validate_all_questions

> serde_json::Value task_validate_all_questions()
Task Validate All Questions

Dispatch an async validation of all questions.

### Parameters

This endpoint does not need any parameter.

### Return type

[**serde_json::Value**](serde_json::Value.md)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)


## task_validate_question

> serde_json::Value task_validate_question(question_public_id)
Task Validate Question

Dispatch an async validation of a single question.

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**question_public_id** | **String** |  | [required] |

### Return type

[**serde_json::Value**](serde_json::Value.md)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: Not defined
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)
