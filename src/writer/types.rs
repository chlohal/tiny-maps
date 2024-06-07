use osmpbfreader::Tags;

pub enum ElementType {
    Node,
    Way,
    Relation,
    RelationTyped,
}

pub enum KnownRelationTypeTag {
    TypeMultipolygon,
    TypeRoute,
    TypeRouteMaster,
    TypeRestriction,
    TypeBoundary,
    TypePublicTransport,
    TypeDestinationSign,
    TypeWaterway,
    TypeEnforcement,
    TypeConnectivity,
    TypeAssociatedStreet,
    TypeSuperroute,
    TypeSite,
    TypeNetwork,
    TypeBuilding,
    TypeMultilinestring,
    TypeStreet,
    TypeBridge,
    TypeTunnel,
    NaturalWater,
    NaturalWood,
}

impl KnownRelationTypeTag {
    pub fn try_match(tags: &Tags) -> Option<Self> {
        use KnownRelationTypeTag::*;
        for (k, v) in tags.iter() {
            match (k.as_str(), v.as_str()) {
                ("type", "multipolygon") => return Some(TypeMultipolygon),
                ("type", "route") => return Some(TypeRoute),
                ("type", "route_master") => return Some(TypeRouteMaster),
                ("type", "restriction") => return Some(TypeRestriction),
                ("type", "boundary") => return Some(TypeBoundary),
                ("type", "public_transport") => return Some(TypePublicTransport),
                ("type", "destination_sign") => return Some(TypeDestinationSign),
                ("type", "waterway") => return Some(TypeWaterway),
                ("type", "enforcement") => return Some(TypeEnforcement),
                ("type", "connectivity") => return Some(TypeConnectivity),
                ("type", "associatedStreet") => return Some(TypeAssociatedStreet),
                ("type", "superroute") => return Some(TypeSuperroute),
                ("type", "site") => return Some(TypeSite),
                ("type", "network") => return Some(TypeNetwork),
                ("type", "building") => return Some(TypeBuilding),
                ("type", "multilinestring") => return Some(TypeMultilinestring),
                ("type", "street") => return Some(TypeStreet),
                ("type", "bridge") => return Some(TypeBridge),
                ("type", "tunnel") => return Some(TypeTunnel),
                ("natural", "water") => return Some(NaturalWater),
                ("natural", "wood") => return Some(NaturalWood),
                _ => {}
            }
        }
        None
    }

    pub fn discriminant(&self) -> u8 {
        match self {
            KnownRelationTypeTag::TypeMultipolygon => 0,
            KnownRelationTypeTag::TypeRoute => 1,
            KnownRelationTypeTag::TypeRouteMaster => 2,
            KnownRelationTypeTag::TypeRestriction => 3,
            KnownRelationTypeTag::TypeBoundary => 4,
            KnownRelationTypeTag::TypePublicTransport => 5,
            KnownRelationTypeTag::TypeDestinationSign => 6,
            KnownRelationTypeTag::TypeWaterway => 7,
            KnownRelationTypeTag::TypeEnforcement => 8,
            KnownRelationTypeTag::TypeConnectivity => 9,
            KnownRelationTypeTag::TypeAssociatedStreet => 10,
            KnownRelationTypeTag::TypeSuperroute => 11,
            KnownRelationTypeTag::TypeSite => 12,
            KnownRelationTypeTag::TypeNetwork => 13,
            KnownRelationTypeTag::TypeBuilding => 14,
            KnownRelationTypeTag::TypeMultilinestring => 15,
            KnownRelationTypeTag::TypeStreet => 16,
            KnownRelationTypeTag::TypeBridge => 17,
            KnownRelationTypeTag::TypeTunnel => 18,
            KnownRelationTypeTag::NaturalWater => 19,
            KnownRelationTypeTag::NaturalWood => 20,
        }
    }
}

impl ElementType {
    pub fn discriminant(&self) -> u8 {
        match self {
            ElementType::Node => 0b00,
            ElementType::Way => 0b01,
            ElementType::Relation => 0b10,
            ElementType::RelationTyped => 0b11,
        }
    }
}
