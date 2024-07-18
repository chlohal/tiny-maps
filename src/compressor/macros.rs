macro_rules! enum_impl
 {
    {
        enum $name:ty;

        fn $fn_name:ident;
        
        $(impl $innr:pat => $var_inner:block )*
    
    } => {
        impl $name {
            fn $fn_name(&self) {
                match self {
                    $(
                        $innr => $var_inner 
                    )*
                }
            }
        }
    };
}

pub(crate) use enum_impl;

macro_rules! make_tag_mapper {
    (
        enum $enum_typename:ident($repr:ident) ;
        $($key:ident = $val:ident)*
    ) => {
        #[repr($repr)]
        #[allow(non_camel_case_types)]
        enum $enum_typename {
            $($val),*
        }

        impl TryFrom<&str> for BuildingType {
            type Error = ();
            fn try_from(value: &str) -> Result<BuildingType, ()>{
                use $enum_typename::*;
                let value = value.as_ref();
                $(
                    if value == stringify!($val)  {
                        return Ok($val);
                    }
                )*
                Err(())
            }
        }
    };
}

pub(crate) use make_tag_mapper;

macro_rules! str_impl
{
    {
        $tr_vis:vis trait $trait_name:ident;
        fn $fn_name:ident( $($argname:ident : $argtype:ty),* ) -> Option<$ret_type:ty>;
        
        $(impl $innr:pat => $var_inner:block )*
    
    } => {
        $tr_vis trait $trait_name {
            fn $fn_name(&self $( , $argname: $argtype )*) -> Option<$ret_type>;
        }

        impl $trait_name for &str {
            fn $fn_name(&self $( , $argname: $argtype )*) -> Option<$ret_type> {
                match *self {
                    $(
                        $innr => Some($var_inner),
                    )*
                    _ => None
                }
            }
        }
    };
}

pub(crate) use str_impl;