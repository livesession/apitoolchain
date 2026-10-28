<!-- Generated from the API reference. Do not edit by hand. -->

# `createNote`

`POST /notes`

Creates a note. Fields beyond these are stored as given.

## Request body

Content type: `application/json`. Required.

| Name | Type | Required | Description |
|---|---|---|---|
| `title` | string | yes | The title. |

## Responses

| Status | Content type | Type | Description |
|---|---|---|---|
| 204 | — | — | The note was created. |

## Example request

```shell
curl --request POST \
     --url https://api.example.com/v1/notes \
     --header 'content-type: application/json' \
     --data '{"title":"string"}'
```
