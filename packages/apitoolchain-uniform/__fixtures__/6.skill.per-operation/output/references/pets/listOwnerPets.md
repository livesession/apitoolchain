<!-- Generated from the API reference. Do not edit by hand. -->

# `listOwnerPets` — List owner pets

`GET /owners/{name}/pets`

Returns the pets of one owner, e.g. the ones a shop assistant looks
after. Pets come sorted by name.

## Path parameters

| Name | Type | Required | Description |
|---|---|---|---|
| `name` | string | yes | The owner's full name. |

## Query parameters

| Name | Type | Required | Description |
|---|---|---|---|
| `name` | string | no | Only pets with this name. |

## Example request

```shell
curl --request GET \
     --url https://api.example.com/v1/owners/string/pets
```
