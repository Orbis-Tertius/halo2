use std::iter;

use ff::Field;

use crate::{
    arithmetic::CurveAffine,
    plonk::{Error, VerifyingKey},
    poly::{
        commitment::{Params},
        multiopen::VerifierQuery,
    },
    transcript::{read_n_points, EncodedChallenge, TranscriptRead},
};

use super::super::{ChallengeX, ChallengeY};
use super::Argument;

pub struct Committed<C> {
    random_poly_commitment: C,
}

pub struct Constructed<C> {
    h_commitments: Vec<C>,
    random_poly_commitment: C,
}

pub struct PartiallyEvaluated<C, S> {
    h_commitments: Vec<C>,
    random_poly_commitment: C,
    random_eval: S,
}

pub struct Evaluated<'params, C, S> {
    h_commitment: C,
    random_poly_commitment: C,
    expected_h_eval: S,
    random_eval: S,
}

impl<C> Argument<C> {
    pub(in crate::plonk) fn read_commitments_before_y<
        E: EncodedChallenge<C>,
        T: TranscriptRead<C, E>,
    >(
        transcript: &mut T,
    ) -> Result<Committed<C>, Error> {
        let random_poly_commitment = transcript.read_point()?;

        Ok(Committed {
            random_poly_commitment,
        })
    }
}

impl<C> Committed<C> {
    pub(in crate::plonk) fn read_commitments_after_y<
        E: EncodedChallenge<C>,
        T: TranscriptRead<C, E>,
    >(
        self,
        vk: &VerifyingKey<C>,
        transcript: &mut T,
    ) -> Result<Constructed<C>, Error> {
        // Obtain a commitment to h(X) in the form of multiple pieces of degree n - 1
        let h_commitments = read_n_points(transcript, vk.domain.get_quotient_poly_degree())?;

        Ok(Constructed {
            h_commitments,
            random_poly_commitment: self.random_poly_commitment,
        })
    }
}

impl<C> Constructed<C> {
    pub(in crate::plonk) fn evaluate_after_x<E: EncodedChallenge<C>, T: TranscriptRead<C, E>>(
        self,
        transcript: &mut T,
    ) -> Result<PartiallyEvaluated<C>, Error> {
        let random_eval = transcript.read_scalar()?;

        Ok(PartiallyEvaluated {
            h_commitments: self.h_commitments,
            random_poly_commitment: self.random_poly_commitment,
            random_eval,
        })
    }
}

impl<C, S> PartiallyEvaluated<C, S> {
    pub(in crate::plonk) fn verify(
        self,
        params: &Params<C, S>,
        expressions: impl Iterator<Item = S>,
        y: ChallengeY<C>,
        xn: S,
    ) -> Evaluated<C> {
        let expected_h_eval = expressions.fold(C::Scalar::zero(), |h_eval, v| h_eval * &*y + &v);
        let expected_h_eval = expected_h_eval * ((xn - C::Scalar::one()).invert().unwrap());

        let h_commitment =
            self.h_commitments
                .iter()
                .rev()
                .fold(params.empty_msm(), |mut acc, commitment| {
                    acc.scale(xn);
                    acc.append_term(C::Scalar::one(), *commitment);
                    acc
                });

        Evaluated {
            expected_h_eval,
            h_commitment,
            random_poly_commitment: self.random_poly_commitment,
            random_eval: self.random_eval,
        }
    }
}

impl<'params, C> Evaluated<'params, C> {
    pub(in crate::plonk) fn queries<'r>(
        &'r self,
        x: ChallengeX<C>,
    ) -> impl Iterator<Item = VerifierQuery<'r, 'params, C>> + Clone
    where
        'params: 'r,
    {
        iter::empty()
            .chain(Some(VerifierQuery::new_msm(
                &self.h_commitment,
                *x,
                self.expected_h_eval,
            )))
            .chain(Some(VerifierQuery::new_commitment(
                &self.random_poly_commitment,
                *x,
                self.random_eval,
            )))
    }
}
