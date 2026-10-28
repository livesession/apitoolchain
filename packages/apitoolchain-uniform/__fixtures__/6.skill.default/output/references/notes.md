<!-- Generated from the API reference. Do not edit by hand. -->

# notes

## `createNote`

`POST /notes`

Creates a note. Fields beyond these are stored as given.

### Request body

Content type: `application/json`. Required.

| Name | Type | Required | Description |
|---|---|---|---|
| `title` | string | yes | The title. |

### Responses

| Status | Content type | Type | Description |
|---|---|---|---|
| 204 | — | — | The note was created. |

### Example request

```shell
curl --request POST \
     --url https://api.example.com/v1/notes \
     --header 'content-type: application/json' \
     --data '{"title":"string"}'
```

## `labelNote`

`PUT /notes/{id}/labels`

Sets the labels of a note. Every extra field must be a string.

### Path parameters

| Name | Type | Required | Description |
|---|---|---|---|
| `id` | string | yes | The note ID. |

### Request body

Content type: `application/json`. Required.

| Name | Type | Required | Description |
|---|---|---|---|
| `id` | string | no | A label set ID. |
| `color` | string | no |  |

### Responses

| Status | Content type | Type | Description |
|---|---|---|---|
| 204 | — | — | The labels were set. |

### Example request

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

## `patchNote`

`PATCH /notes/{id}`

Merges fields into a note.

### Path parameters

| Name | Type | Required | Description |
|---|---|---|---|
| `id` | string | yes | The note ID. |

### Request body

Content type: `application/json`. Required.

### Responses

| Status | Content type | Type | Description |
|---|---|---|---|
| 204 | — | — | The note was updated. |

### Example request

```shell
curl --request PATCH \
     --url https://api.example.com/v1/notes/string \
     --header 'content-type: application/json'
```

## `closeNote`

`POST /notes/{id}/close`

Closes a note. Unknown fields are rejected.

### Path parameters

| Name | Type | Required | Description |
|---|---|---|---|
| `id` | string | yes | The note ID. |

### Request body

Content type: `application/json`. Required.

| Name | Type | Required | Description |
|---|---|---|---|
| `reason` | string | no | Why. |

### Responses

| Status | Content type | Type | Description |
|---|---|---|---|
| 204 | — | — | The note was closed. |

### Example request

```shell
curl --request POST \
     --url https://api.example.com/v1/notes/string/close \
     --header 'content-type: application/json' \
     --data '{"reason":"string"}'
```
