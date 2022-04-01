#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use frame_support::{
		sp_runtime::traits::Hash,
		traits::{ Randomness, Currency, tokens::ExistenceRequirement },
		transactional
	};
	use sp_io::hashing::blake2_128;
	use scale_info::TypeInfo;

	#[cfg(feature = "std")]
	use frame_support::serde::{Deserialize, Serialize};

	type AccountOf<T> = <T as frame_system::Config>::AccountId;
	type BalanceOf<T> =
		<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

	// Struct for holding NFT information.
	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	#[scale_info(skip_type_params(T))]
	#[codec(mel_bound())]
	pub struct InternetCashNFT<T: Config> {
		pub dna: [u8; 16],   // Using 16 bytes to represent a NFT DNA
		pub price: Option<BalanceOf<T>>,
		pub gender: Gender,
		pub owner: AccountOf<T>,
		pub creator: AccountOf<T>,
		pub royalty: Option<BalanceOf<T>>,
	}
	// Enum declaration for Gender.h\
	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
	pub enum Gender {
		Male,
		Female,
	}

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

	/// Configure the pallet by specifying the parameters and types it depends on.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// The Currency handler for the Intertnetcashnft pallet.
		type Currency: Currency<Self::AccountId>;

		/// The maximum amount of NFTs a single account can own.
		#[pallet::constant]
		type MaxNFTsOwned: Get<u32>;

		/// The type of Randomness we want to specify for this pallet.
		type NFTRandomness: Randomness<Self::Hash, Self::BlockNumber>;
	}

	// Errors.
	#[pallet::error]
	pub enum Error<T> {
		/// Handles arithmetic overflow when incrementing the NFT counter.
        NFTCntOverflow,
        /// An account cannot own more NFT than `MaxNFTCount`.
        ExceedMaxNFTOwned,
        /// Buyer cannot be the owner.
        BuyerIsNFTOwner,
        /// Cannot transfer a NFT to its owner.
        TransferToSelf,
        /// Handles checking whether the NFT exists.
        NFTNotExist,
        /// Handles checking that the NFT is owned by the account transferring, buying or setting a price for it.
        NotNFTOwner,
        /// Ensures the NFT is for sale.
        NFTNotForSale,
        /// Ensures that the buying price is greater than the asking price.
        NFTBidPriceTooLow,
        /// Ensures that an account has enough funds to purchase a NFT.
        NotEnoughBalance,
        //Ensre theat the NFT does nnot exist
        NFTExists,
		// Owner Can't set creator royalty
		NotCreator, 
	}

	// Events.
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new NFT was successfully created. \[sender, NFT_id\]
        Created(T::AccountId, T::Hash),
        /// NFT price was successfully set. \[sender, NFT_id, new_price\]
        PriceSet(T::AccountId, T::Hash, Option<BalanceOf<T>>),
        /// A NFT was successfully transferred. \[from, to, NFT_id\]
        Transferred(T::AccountId, T::AccountId, T::Hash),
        /// A NFT was successfully bought. \[buyer, seller, _id, bid_price\]
        Bought(T::AccountId, T::AccountId, T::Hash, BalanceOf<T>),
		/// Caretor Royalty for NFT Was successfully set
		RoyaltySet(T::AccountId, T::Hash, Option<BalanceOf<T>>),
	}

	// Storage items.
	#[pallet::storage]
	#[pallet::getter(fn nft_cnt)]
	/// Keeps track of the number of NFTs in existence.
	pub(super) type NFTCnt<T: Config> = StorageValue<_, u64, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn nfts)]
	/// Stores a nft's unique traits, owner and price.
	pub(super) type Nfts<T: Config> = StorageMap<_, Twox64Concat, T::Hash, InternetCashNFT<T>>;

	#[pallet::storage]		
	#[pallet::getter(fn nfts_owned)]
	/// Keeps track of what accounts own what NFT.
	pub(super) type 
	NFTsOwned<T: Config> =
		StorageMap<_, Twox64Concat, T::AccountId, BoundedVec<T::Hash, T::MaxNFTsOwned>, ValueQuery>;

	// TODO Part IV: Our pallet's genesis configuration.

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new unique NFT.
		///
		/// The actual NFT creation is done in the `mint()` function.
		#[pallet::weight(100)]
		pub fn create_nft(origin: OriginFor<T>) -> DispatchResult {
            let sender = ensure_signed(origin)?; 
            let nft_id = Self::mint(&sender, None, None)?; 
            // Logging to the console
            log::info!("A NFT is created with ID: {:?}.", nft_id);

            //Emmiting the Created event
            Self::deposit_event(Event::Created(sender, nft_id));

			Ok(())
		}
		#[pallet::weight(100)]
		pub fn set_price(
			origin: OriginFor<T>,
			nft_id: T::Hash,
			new_price: Option<BalanceOf<T>>,
		  ) -> DispatchResult {

			let sender = ensure_signed(origin)?;
			ensure!(Self::is_nft_owner(&nft_id, &sender)?, <Error<T>>::NotNFTOwner);

			//If above condition passes, set the NFT price
			let mut nft = Self::nfts(&nft_id).ok_or(<Error<T>>::NFTNotExist)?;
			nft.price = new_price.clone();
			<Nfts<T>>::insert(&nft_id, nft);

			// Emit Price set event after setting the price 
			Self::deposit_event(Event::PriceSet(sender, nft_id, new_price));
			Ok(())

		}
		//Function to set creator royalty for every resell
		#[pallet::weight(100)]
		pub fn set_creator_royalty(
			origin: OriginFor<T>,
			nft_id: T::Hash,
			royalty: Option<BalanceOf<T>>
		)-> DispatchResult {

			let sender = ensure_signed(origin)?;
			ensure!(Self::is_creator(&nft_id, &sender)?, <Error<T>>::NotCreator);

			//If above condition passes, check for NFT existance 
			let mut nft = Self::nfts(&nft_id).ok_or(<Error<T>>::NFTNotExist)?;

			//If NFT Exist set the creator royalty
			nft.royalty = royalty.clone();
			<Nfts<T>>::insert(&nft_id, nft);

			// Emit Price set event after setting the price 
			Self::deposit_event(Event::RoyaltySet(sender, nft_id, royalty));
			Ok(())

		}

		//Transfer NFT Function 
		#[pallet::weight(100)]
		pub fn transfer(
			origin: OriginFor<T>,
			to: T::AccountId,
			nft_id: T::Hash
		) -> DispatchResult {
			let from = ensure_signed(origin)?;

			// Ensure the NFT exists and is called by the NFT owner
			ensure!(Self::is_nft_owner(&nft_id, &from)?, <Error<T>>::NotNFTOwner);

			// Verify the NFT is not transferring back to its owner.
			ensure!(from != to, <Error<T>>::TransferToSelf);

			// Verify the recipient has the capacity to receive one more NFT
			let to_owned = <NFTsOwned<T>>::get(&to);
			ensure!((to_owned.len() as u32) < T::MaxNFTsOwned::get(), <Error<T>>::ExceedMaxNFTOwned);

			Self::transfer_nft_to(&nft_id, &to)?;

			Self::deposit_event(Event::Transferred(from, to, nft_id));

			Ok(())
		}
		// buy_nft
		#[transactional]
		#[pallet::weight(100)]
		pub fn buy_nft(
			origin: OriginFor<T>,
			nft_id: T::Hash,
			bid_price: BalanceOf<T>
		) -> DispatchResult {
			let buyer = ensure_signed(origin)?;
	
			// Check the nft exists and buyer is not the current nft owner
			let nft = Self::nfts(&nft_id).ok_or(<Error<T>>::NFTNotExist)?;
			ensure!(nft.owner != buyer, <Error<T>>::BuyerIsNFTOwner);
			
			//Check if the nft is for sale.
			// Check the nft is for sale and the nft ask price <= bid_price
			if let Some(ask_price) = nft.price {
				ensure!(ask_price <= bid_price, <Error<T>>::NFTBidPriceTooLow);
			} else {
				Err(<Error<T>>::NFTNotForSale)?;
			}
			// Check the buyer has enough free balance
			ensure!(T::Currency::free_balance(&buyer) >= bid_price, <Error<T>>::NotEnoughBalance);

			// Verify the buyer has the capacity to receive one more nft
			let to_owned = <NFTsOwned<T>>::get(&buyer);
			ensure!((to_owned.len() as u32) < T::MaxNFTsOwned::get(), <Error<T>>::ExceedMaxNFTOwned);
			let seller = nft.owner.clone();
			let creator = nft.creator.clone();
			
			let creator_share = nft.royalty.map(|creator_royalty| {
				nft.price.unwrap_or_default() * creator_royalty / 100u32.into()
			});

			//Transfer the creator share to creator's address
			if let Some(creator_share) = creator_share{
				T::Currency::transfer(&buyer, &creator, creator_share, ExistenceRequirement::KeepAlive)?;	
			};
			//Transfer the owner share to owner's address
			let owner_share = nft.royalty.map(|creator_royalty|{ 
				nft.price.unwrap_or_default() - (nft.price.unwrap_or_default() * creator_royalty / 100u32.into())
			});

			if let Some(owner_share) = owner_share{
				T::Currency::transfer(&buyer, &creator, owner_share, ExistenceRequirement::KeepAlive)?;	
			}

			//Transfer the amount from buyer to seller
			// T::Currency::transfer(&buyer, &seller, bid_price, ExistenceRequirement::KeepAlive)?;
			// Transfer the nft from seller to buyer
			Self::transfer_nft_to(&nft_id, &buyer)?;

			// Deposit relevant Event
			Self::deposit_event(Event::Bought(buyer, seller, nft_id, bid_price));
	
			Ok(())
		}

		// TODO Part IV: breed_NFT
	}

	//** Our helper functions.**//

	impl<T: Config> Pallet<T> {
		// Generate a random gender value
		fn gen_gender() -> Gender {
			let random = T::NFTRandomness::random(&b"gender"[..]).0;
			match random.as_ref()[0] % 2 {
				0 => Gender::Male,
				_ => Gender::Female,
			}
		}

		// Generate a random DNA value
		fn gen_dna() -> [u8; 16] {
			let payload = (
				T::NFTRandomness::random(&b"dna"[..]).0,
				<frame_system::Pallet<T>>::extrinsic_index().unwrap_or_default(),
				<frame_system::Pallet<T>>::block_number(),
			);
			payload.using_encoded(blake2_128)
		}

		// Create new DNA with existing DNA
		pub fn breed_dna(parent1: &T::Hash, parent2: &T::Hash) -> Result<[u8; 16], Error<T>> {
			let dna1 = Self::nfts(parent1).ok_or(<Error<T>>::NFTNotExist)?.dna;
			let dna2 = Self::nfts(parent2).ok_or(<Error<T>>::NFTNotExist)?.dna;

			let mut new_dna = Self::gen_dna();
			for i in 0..new_dna.len() {
				new_dna[i] = (new_dna[i] & dna1[i]) | (!new_dna[i] & dna2[i]);
			}
			Ok(new_dna)
		}

        //helper function to mint NFTs
		pub fn mint(
            owner: &T::AccountId,
            dna: Option<[u8; 16]>,
            gender: Option<Gender>,
        ) -> Result<T::Hash, Error<T>> {
            let nft = InternetCashNFT::<T> {
                dna: dna.unwrap_or_else(Self::gen_dna),
                price: None,
                gender: gender.unwrap_or_else(Self::gen_gender),
                owner: owner.clone(),
				creator: owner.clone(),
				royalty: None,
			};
        
            let nft_id = T::Hashing::hash_of(&nft);
        
            // Performs this operation first as it may fail
            let new_cnt = Self::nft_cnt().checked_add(1)
                .ok_or(<Error<T>>::NFTCntOverflow)?;
        
            // Check if the NFT does not already exist in our storage map
            ensure!(Self::nfts(&nft_id) == None, <Error<T>>::NFTExists);
        
            // Performs this operation first because as it may fail
            <NFTsOwned<T>>::try_mutate(&owner, |nft_vec| {
                nft_vec.try_push(nft_id)
            }).map_err(|_| <Error<T>>::ExceedMaxNFTOwned)?;
        
            <Nfts<T>>::insert(nft_id, nft);
            <NFTCnt<T>>::put(new_cnt);
            Ok(nft_id)
        }

		// Helper to check correct NFT owner
		pub fn is_nft_owner(nft_id: &T::Hash, acct: &T::AccountId) -> Result<bool, Error<T>> {
			match Self::nfts(nft_id) {
				Some(nft) => Ok(nft.owner == *acct),
				None => Err(<Error<T>>::NFTNotExist)
			}
		}

		//Helper function to check actual creator of NFT
		pub fn is_creator(nft_id: &T::Hash, acct: &T::AccountId) -> Result<bool, Error<T>> {
			match Self::nfts(nft_id) {
				Some(nft) => Ok(nft.creator == *acct),
				None => Err(<Error<T>>::NotCreator)
			}
		}

		// Helper function to transfer nft
		#[transactional]
		pub fn transfer_nft_to(
			nft_id: &T::Hash,
			to: &T::AccountId,
		) -> Result<(), Error<T>> {
			let mut token = Self::nfts(&nft_id).ok_or(<Error<T>>::NFTNotExist)?;

			let prev_owner = token.owner.clone();

			// Remove `NFT_ID` from the NFTOwned vector of `prev_NFT_owner`
			<NFTsOwned<T>>::try_mutate(&prev_owner, |owned| {
				if let Some(ind) = owned.iter().position(|&id| id == *nft_id) {
					owned.swap_remove(ind);
					return Ok(());
				}
				Err(())
			}).map_err(|_| <Error<T>>::NFTNotExist)?;

			// Update the NFT owner
			token.owner = to.clone();
			// Reset the ask price so the NFT is not for sale until `set_price()` is called
			// by the current owner.
		 	token.price = None;

			<Nfts<T>>::insert(nft_id, token);

			<NFTsOwned<T>>::try_mutate(to, |vec| {
				vec.try_push(*nft_id)
			}).map_err(|_| <Error<T>>::ExceedMaxNFTOwned)?;

			Ok(())
		}
		//function to  convert balance to u64
		pub fn balance_to_u64(input: BalanceOf<T>) -> Option<u64> {
			TryInto::<u64>::try_into(input).ok()
		}
	
	}
}