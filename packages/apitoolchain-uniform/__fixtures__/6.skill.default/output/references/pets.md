<!-- Generated from the API reference. Do not edit by hand. -->

# pets

## `listPets` — List pets

`GET /pets`

Returns a page of pets, newest
first. Filters combine with AND.

Use `cursor` from a previous page to continue:
- pass `next_cursor` back as `cursor`
- keep the other filters as they were

### Query parameters

| Name | Type | Required | Description |
|---|---|---|---|
| `limit` | integer | no | Maximum number of pets to return, at most 100. Default: `20`. Minimum: `1`. Maximum: `100`. |
| `status` | string | no | Only pets with this status. One of: `available`, `pending`, `sold`. |
| `tag` | array of string | no | Only pets carrying every one of these tags. |
| `cursor` | string | no | The `next_cursor` of the previous page. |
| `sort` | string | no | Deprecated. Sort order (use `order` instead). |

### Responses

| Status | Content type | Type | Description |
|---|---|---|---|
| 200 | `application/json` | PetList | A page of pets. |
| 400 | `application/json` | Error | The filters are invalid. |

#### 200 — `application/json`

| Name | Type | Description |
|---|---|---|
| `data` | array of Pet |  |
| `data[].id` | string | Unique identifier for the pet. |
| `data[].name` | string | The pet's name. |
| `data[].status` | string | Where the pet is in the sale. One of: `available`, `pending`, `sold`. |
| `data[].tags` | array of string |  |
| `data[].owner` | Owner | Who looks after the pet. |
| `has_more` | boolean | Whether another page follows. |
| `next_cursor` | string or null | Pass as `cursor` to fetch the next page. |

### Example request

```shell
curl --request GET \
     --url https://api.example.com/v1/pets \
     --header 'accept: application/json'
```

## `createPet` — Create pet

`POST /pets`

Creates a pet. Names need not be unique | pipes are kept.

### Request body

Content type: `application/json`. Required.

| Name | Type | Required | Description |
|---|---|---|---|
| `name` | string | yes | The pet's name. Example: `Rex`. |
| `status` | string | no | One of: `available`, `pending`, `sold`. Default: `available`. |
| `tags` | array of string | no | Free-form labels. |
| `owner` | Owner | no | Who looks after the pet. |
| `owner.name` | string | yes | The owner's full name. |
| `owner.contact` | object | no | How to reach the owner. |
| `vaccinated` | boolean | no | Default: `false`. |
| `priority` | integer | no | Listing priority, 1 the highest. One of: `1`, `2`, `3`. |
| `size` | string or null | no | The pet's size; null when unknown. One of: `small`, `medium`, `large`. |
| `attributes` | object | no | Free-form attributes ==================== Stored as given, for example: ```json { "color": "brown", "legs": 4 } ``` Keys are case-sensitive. |

### Responses

| Status | Content type | Type | Description |
|---|---|---|---|
| 201 | `application/json` | Pet | The pet that was created. |
| 422 | `application/json` | Error | The pet is invalid. |

#### 201 — `application/json`

| Name | Type | Description |
|---|---|---|
| `id` | string | Unique identifier for the pet. |
| `name` | string | The pet's name. |
| `status` | string | Where the pet is in the sale. One of: `available`, `pending`, `sold`. |
| `tags` | array of string |  |
| `owner` | Owner | Who looks after the pet. |
| `owner.name` | string | The owner's full name. |
| `owner.contact` | object | How to reach the owner. |

### Example request

```shell
curl --request POST \
     --url https://api.example.com/v1/pets \
     --header 'accept: application/json' \
     --header 'content-type: application/json' \
     --data '
{
  "name": "Rex",
  "status": "available",
  "tags": [
    "string"
  ],
  "owner": {
    "name": "string",
    "contact": {
      "email": "string",
      "phone": "string"
    }
  },
  "vaccinated": false,
  "priority": 1,
  "size": "small"
}
'
```

## `searchPets` — Search pets

`POST /pets/search`

Searches pets by name. A read-only POST — the query is too rich for a URL.

### Request body

Content type: `application/json`. Required.

| Name | Type | Required | Description |
|---|---|---|---|
| `query` | string | yes | Text to match against pet names. Example: `rex`. |
| `filters` | object | no | Narrow the matches; every filter must hold: - status - tags |
| `filters.status` | string | no | One of: `available`, `pending`, `sold`. |
| `filters.tags` | array of string | no |  |
| `limit` | integer | no | Maximum number of matches, 100 by default and at most 5000. Larger values are rejected. Default: `100`. Maximum: `5000`. |

### Responses

| Status | Content type | Type | Description |
|---|---|---|---|
| 200 | `application/json` | PetList | The matching pets. |

#### 200 — `application/json`

| Name | Type | Description |
|---|---|---|
| `data` | array of Pet |  |
| `data[].id` | string | Unique identifier for the pet. |
| `data[].name` | string | The pet's name. |
| `data[].status` | string | Where the pet is in the sale. One of: `available`, `pending`, `sold`. |
| `data[].tags` | array of string |  |
| `data[].owner` | Owner | Who looks after the pet. |
| `has_more` | boolean | Whether another page follows. |
| `next_cursor` | string or null | Pass as `cursor` to fetch the next page. |

### Example request

```shell
curl --request POST \
     --url https://api.example.com/v1/pets/search \
     --header 'accept: application/json' \
     --header 'content-type: application/json' \
     --data '
{
  "query": "rex",
  "filters": {
    "status": "available",
    "tags": [
      "string"
    ]
  },
  "limit": 100
}
'
```

## `getPet` — Retrieve pet

`GET /pets/{petId}`

Returns one pet.

### Path parameters

| Name | Type | Required | Description |
|---|---|---|---|
| `petId` | string | yes | The pet's ID. Example: `pet_123`. |

### Responses

| Status | Content type | Type | Description |
|---|---|---|---|
| 200 | `application/json` | Pet | The pet. |
| 404 | `application/json` | Error | No pet has this ID. |

#### 200 — `application/json`

| Name | Type | Description |
|---|---|---|
| `id` | string | Unique identifier for the pet. |
| `name` | string | The pet's name. |
| `status` | string | Where the pet is in the sale. One of: `available`, `pending`, `sold`. |
| `tags` | array of string |  |
| `owner` | Owner | Who looks after the pet. |
| `owner.name` | string | The owner's full name. |
| `owner.contact` | object | How to reach the owner. |

### Example request

```javascript
const pet = await client.pets.retrieve("pet_123");
```

## `updatePet` — Update pet

`PATCH /pets/{petId}`

Updates the fields given; the others keep their values.

### Path parameters

| Name | Type | Required | Description |
|---|---|---|---|
| `petId` | string | yes | The pet's ID. |

### Request body

Content type: `application/json`. Required.

| Name | Type | Required | Description |
|---|---|---|---|
| `name` | string | no | The new name. |
| `status` | string | no | One of: `available`, `pending`, `sold`. |
| `weight` | number or null | no | Weight in kilograms; null clears it. |

### Responses

| Status | Content type | Type | Description |
|---|---|---|---|
| 200 | `application/json` | Pet | The updated pet. |

#### 200 — `application/json`

| Name | Type | Description |
|---|---|---|
| `id` | string | Unique identifier for the pet. |
| `name` | string | The pet's name. |
| `status` | string | Where the pet is in the sale. One of: `available`, `pending`, `sold`. |
| `tags` | array of string |  |
| `owner` | Owner | Who looks after the pet. |
| `owner.name` | string | The owner's full name. |
| `owner.contact` | object | How to reach the owner. |

### Example request

```shell
curl --request PATCH \
     --url https://api.example.com/v1/pets/string \
     --header 'accept: application/json' \
     --header 'content-type: application/json' \
     --data '
{
  "name": "string",
  "status": "available",
  "weight": 0
}
'
```

## `deletePet` — Delete pet

`DELETE /pets/{petId}`

Deletes a pet for good.

### Path parameters

| Name | Type | Required | Description |
|---|---|---|---|
| `petId` | string | yes | The pet's ID. |

### Responses

| Status | Content type | Type | Description |
|---|---|---|---|
| 204 | — | — | The pet was deleted. |
| 404 | `application/json` | Error | No pet has this ID. |

### Example request

```shell
curl --request DELETE \
     --url https://api.example.com/v1/pets/string \
     --header 'accept: application/json'
```

## `replacePetTags` — Replace pet tags

`PUT /pets/{petId}/tags`

Replaces every tag of a pet with the list given.

### Path parameters

| Name | Type | Required | Description |
|---|---|---|---|
| `petId` | string | yes | The pet's ID. |

### Request body

Content type: `application/json`. Required.

| Name | Type | Required | Description |
|---|---|---|---|
| (body) | array | yes |  |

### Responses

| Status | Content type | Type | Description |
|---|---|---|---|
| 200 | `application/json` | Pet | The pet with its new tags. |

#### 200 — `application/json`

| Name | Type | Description |
|---|---|---|
| `id` | string | Unique identifier for the pet. |
| `name` | string | The pet's name. |
| `status` | string | Where the pet is in the sale. One of: `available`, `pending`, `sold`. |
| `tags` | array of string |  |
| `owner` | Owner | Who looks after the pet. |
| `owner.name` | string | The owner's full name. |
| `owner.contact` | object | How to reach the owner. |

### Example request

```shell
curl --request PUT \
     --url https://api.example.com/v1/pets/string/tags \
     --header 'accept: application/json' \
     --header 'content-type: application/json' \
     --data '["string"]'
```

## `listOwnerPets` — List owner pets

`GET /owners/{name}/pets`

Returns the pets of one owner, e.g. the ones a shop assistant looks
after. Pets come sorted by name.

### Path parameters

| Name | Type | Required | Description |
|---|---|---|---|
| `name` | string | yes | The owner's full name. |

### Query parameters

| Name | Type | Required | Description |
|---|---|---|---|
| `name` | string | no | Only pets with this name. |

### Example request

```shell
curl --request GET \
     --url https://api.example.com/v1/owners/string/pets
```

## `uploadPetPhoto` — Upload pet photo

`POST /pets/{petId}/photo`

Uploads a photo of the pet.

### Path parameters

| Name | Type | Required | Description |
|---|---|---|---|
| `petId` | string | yes | The pet's ID. |

### Request body

Content type: `multipart/form-data`. Required.

| Name | Type | Required | Description |
|---|---|---|---|
| `file` | string | yes | The image. |
| `caption` | string | no |  |

### Responses

| Status | Content type | Type | Description |
|---|---|---|---|
| 204 | — | — | The photo was stored. |

### Example request

```shell
curl --request POST \
     --url https://api.example.com/v1/pets/string/photo \
     --header 'content-type: multipart/form-data' \
     --form file=string \
     --form caption=string
```
