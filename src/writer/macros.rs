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