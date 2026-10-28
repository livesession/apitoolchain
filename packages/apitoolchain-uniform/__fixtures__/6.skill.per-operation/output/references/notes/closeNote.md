<!-- Generated from the API reference. Do not edit by hand. -->

# `closeNote`

`POST /notes/{id}/close`

Closes a note. Unknown fields are rejected.

## Path parameters

| Name | Type | Required | Description |
|---|---|---|---|
| `id` | string | yes | The note ID. |

## Request body

Content type: `application/json`. Required.

| Name | Type | Required | Description |
|---|---|---|---|
| `reason` | string | no | Why. |

## Responses

| Status | Content type | Type | Description |
|---|---|---|---|
| 204 | — | — | The note was closed. |

## Example request

```shell
curl --request POST \
     --url https://api.example.com/v1/notes/string/close \
     --header 'content-type: application/json' \
     --data '{"reason":"string"}'
```
