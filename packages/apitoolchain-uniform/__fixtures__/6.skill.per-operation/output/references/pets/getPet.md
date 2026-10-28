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

## Example request

```javascript
const pet = await client.pets.retrieve("pet_123");
```
