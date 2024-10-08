use std::collections::BTreeMap;
use std::fmt;

use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

use crate::parse::ParseFromStr;
use crate::str::WitnessName;
use crate::types::ResolvedType;
use crate::value::Value;
use crate::witness::WitnessValues;

struct WitnessMapVisitor;

impl<'de> de::Visitor<'de> for WitnessMapVisitor {
    type Value = WitnessValues;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a map with string keys and value-map values")
    }

    fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
    where
        M: de::MapAccess<'de>,
    {
        let mut witness = WitnessValues::empty();
        while let Some((key, value)) = access.next_entry()? {
            witness.insert(key, value).map_err(de::Error::custom)?;
        }
        Ok(witness)
    }
}

impl<'de> Deserialize<'de> for WitnessValues {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(WitnessMapVisitor)
    }
}

struct ValueMapVisitor;

impl<'de> de::Visitor<'de> for ValueMapVisitor {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a map with \"value\" and \"type\" fields")
    }

    fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
    where
        M: de::MapAccess<'de>,
    {
        let mut value = None;
        let mut ty = None;

        while let Some(key) = access.next_key::<&str>()? {
            match key {
                "value" => {
                    if value.is_some() {
                        return Err(de::Error::duplicate_field("value"));
                    }
                    value = Some(access.next_value::<&str>()?);
                }
                "type" => {
                    if ty.is_some() {
                        return Err(de::Error::duplicate_field("type"));
                    }
                    ty = Some(access.next_value::<&str>()?);
                }
                _ => {
                    return Err(de::Error::unknown_field(key, &["value", "type"]));
                }
            }
        }

        let ty = match ty {
            Some(s) => ResolvedType::parse_from_str(s).map_err(de::Error::custom)?,
            None => return Err(de::Error::missing_field("type")),
        };
        match value {
            Some(s) => Value::parse_from_str(s, &ty).map_err(de::Error::custom),
            None => Err(de::Error::missing_field("value")),
        }
    }
}

impl<'de> Deserialize<'de> for Value {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(ValueMapVisitor)
    }
}

struct ParserVisitor<A>(std::marker::PhantomData<A>);

impl<'de, A: ParseFromStr> de::Visitor<'de> for ParserVisitor<A> {
    type Value = A;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a valid string")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        A::parse_from_str(value).map_err(E::custom)
    }
}

impl<'de> Deserialize<'de> for WitnessName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(ParserVisitor::<Self>(std::marker::PhantomData))
    }
}

impl Serialize for WitnessValues {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeMap;

        let mut map = serializer.serialize_map(Some(self.len()))?;
        for (name, value) in self.iter() {
            let value_map = BTreeMap::from([
                ("value", value.to_string()),
                ("type", value.ty().to_string()),
            ]);
            map.serialize_entry(name.as_inner(), &value_map)?;
        }
        map.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn witness_serde_duplicate_assignment() {
        let s = r#"{
  "A": { "value": "42", "type": "u32" },
  "A": { "value": "43", "type": "u16" }
}"#;

        match serde_json::from_str::<WitnessValues>(s) {
            Ok(_) => panic!("Duplicate witness assignment was falsely accepted"),
            Err(error) => assert_eq!(
                "Witness `A` has already been assigned a value at line 4 column 1",
                &error.to_string()
            ),
        }
    }
}
