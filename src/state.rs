use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_program::{
    clock::UnixTimestamp,
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack, Sealed},
    pubkey::Pubkey,
};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Auction {
    /// Is `true` if this structure has been initialized
    pub is_initialized: bool,

    // Auction authority.
    pub authority: Pubkey,
    // Token id.
    pub token: Pubkey,

    // Auction start time.
    pub time_start: UnixTimestamp,
    // Time between price changes.
    pub time_step: UnixTimestamp,
    // Initial price per token.
    pub price_start: u64,
    // Price change on each time step.
    pub price_step: u64,
}

impl IsInitialized for Auction {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

impl Pack for Auction {
    const LEN: usize = 97;

    fn pack_into_slice(&self, dst: &mut [u8]) {
        let dst = array_mut_ref![dst, 0, <Auction as Pack>::LEN];
        let (
            is_initialized_dst,
            authority_dst,
            token_dst,
            time_start_dst,
            time_step_dst,
            price_start_dst,
            price_step_dst,
        ) = mut_array_refs![dst, 1, 32, 32, 8, 8, 8, 8];
        let &Auction {
            is_initialized,
            ref authority,
            ref token,
            time_start,
            time_step,
            price_start,
            price_step,
        } = self;
        is_initialized_dst[0] = is_initialized as u8;
        authority_dst.copy_from_slice(authority.as_ref());
        token_dst.copy_from_slice(token.as_ref());
        *time_start_dst = time_start.to_le_bytes();
        *time_step_dst = time_step.to_le_bytes();
        *price_start_dst = price_start.to_le_bytes();
        *price_step_dst = price_step.to_le_bytes();
    }

    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        let src = array_ref![src, 0, <Auction as Pack>::LEN];
        let (is_initialized, authority, token, time_start, time_step, price_start, price_step) =
            array_refs![src, 1, 32, 32, 8, 8, 8, 8];
        let is_initialized = match is_initialized {
            [0] => false,
            [1] => true,
            _ => return Err(ProgramError::InvalidAccountData),
        };
        Ok(Auction {
            is_initialized,
            authority: Pubkey::new_from_array(*authority),
            token: Pubkey::new_from_array(*token),
            time_start: UnixTimestamp::from_le_bytes(*time_start),
            time_step: UnixTimestamp::from_le_bytes(*time_step),
            price_start: u64::from_le_bytes(*price_start),
            price_step: u64::from_le_bytes(*price_step),
        })
    }
}

impl Sealed for Auction {}
