# \ImagesApi

All URIs are relative to *http://localhost*

Method | HTTP request | Description
------------- | ------------- | -------------
[**upload_image**](ImagesApi.md#upload_image) | **POST** /api/v1/images/upload | Upload Image



## upload_image

> models::EnvelopeImageUploadResponse upload_image(image_upload_payload)
Upload Image

### Parameters


Name | Type | Description  | Required | Notes
------------- | ------------- | ------------- | ------------- | -------------
**image_upload_payload** | [**ImageUploadPayload**](ImageUploadPayload.md) |  | [required] |

### Return type

[**models::EnvelopeImageUploadResponse**](Envelope_ImageUploadResponse_.md)

### Authorization

[csrfToken](../README.md#csrfToken), [cookieAuth](../README.md#cookieAuth), [bearerAuth](../README.md#bearerAuth)

### HTTP request headers

- **Content-Type**: application/json
- **Accept**: application/json

[[Back to top]](#) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to Model list]](../README.md#documentation-for-models) [[Back to README]](../README.md)
