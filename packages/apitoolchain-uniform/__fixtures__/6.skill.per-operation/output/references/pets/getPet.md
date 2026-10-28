<!-- Generated from the API reference. Do not edit by hand. -->

# `getPet` — Retrieve pet

`GET /pets/{petId}`

Returns one pet.

## Path parameters

| Name | Type | Required | Description |
|---|---|---|---|
| `petId` | string | yes | The pet's ID. Example: `pet_123`. |

## Responses

| Status | Content type | Type | Description |
|---|---|---|---|
| 200 | `application/json` | Pet | The pet. |
| 404 | `application/json` | Error | No pet has this ID. |

### 200 — `application/json`

| Name | Type | Description |
|---|---|---|
| `id` | string | Unique identifier for the pet. |
| `name` | string | The pet's name. |
| `status` | string | Where the pet is in the sale. One of: `available`, `pending`, `sold`. |
| `tags` | array of string |  |
| `owner` | Owner | Who looks after the pet. |
| `owner.name` | string | The owner's full name. |
| `owner.contact` | object | How to reach the owner. |

## Example request

```javascript
const pet = await client.pets.retrieve("pet_123");
```
