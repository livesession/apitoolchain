<!-- Generated from openapi.yaml (notes) -- regenerate, do not edit -- > -->

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
     --url 'https://api.example.com/v1/notes' \
     --header 'content-type: application/json' \
     --data '{
  "title": "<title>"
}'
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
     --url 'https://api.example.com/v1/notes/<id>/labels' \
     --header 'content-type: application/json' \
     --data '{}'
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
     --url 'https://api.example.com/v1/notes/<id>' \
     --header 'content-type: application/json' \
     --data '{}'
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
     --url 'https://api.example.com/v1/notes/<id>/close' \
     --header 'content-type: application/json' \
     --data '{}'
```
