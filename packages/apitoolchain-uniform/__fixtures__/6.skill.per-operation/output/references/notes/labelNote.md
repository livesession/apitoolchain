<!-- Generated from the API reference. Do not edit by hand. -->

# `labelNote`

`PUT /notes/{id}/labels`

Sets the labels of a note. Every extra field must be a string.

## Path parameters

| Name | Type | Required | Description |
|---|---|---|---|
| `id` | string | yes | The note ID. |

## Request body

Content type: `application/json`. Required.

| Name | Type | Required | Description |
|---|---|---|---|
| `id` | string | no | A label set ID. |
| `color` | string | no |  |

## Responses

| Status | Content type | Type | Description |
|---|---|---|---|
| 204 | — | — | The labels were set. |

## Example request

```shell
curl --request PUT \
     --url https://api.example.com/v1/notes/string/labels \
     --header 'content-type: application/json' \
     --data '
{
  "id": "string",
  "color": "string",
  "property1": "string",
  "property2": "string"
}
'
```
