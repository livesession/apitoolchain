<!-- Generated from the API reference. Do not edit by hand. -->

# `patchNote`

`PATCH /notes/{id}`

Merges fields into a note.

## Path parameters

| Name | Type | Required | Description |
|---|---|---|---|
| `id` | string | yes | The note ID. |

## Request body

Content type: `application/json`. Required.

## Responses

| Status | Content type | Type | Description |
|---|---|---|---|
| 204 | — | — | The note was updated. |

## Example request

```shell
curl --request PATCH \
     --url https://api.example.com/v1/notes/string \
     --header 'content-type: application/json'
```
