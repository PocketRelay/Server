import{S as oe,i as ie,s as ue,a as b,k as p,q as T,c as E,l as _,m as d,r as w,h as f,n as u,b as W,G as s,R as H,M as V,T as ce,g as I,d as X,f as fe,U as pe,y as _e,z as me,A as de,B as he,u as ge,v as ve}from"../chunks/index.6f0c0f63.mjs";import{g as be}from"../chunks/navigation.c243e149.mjs";import{r as Ee,H as Le,t as ye}from"../chunks/api.36344f67.mjs";import{L as Ae}from"../chunks/Loader.f33e46d2.mjs";function Pe(o,t){return Ee({method:Le.POST,route:"auth/login",body:{email:o,password:t}})}function ne(o){let t,a;return t=new Ae({}),{c(){_e(t.$$.fragment)},l(e){me(t.$$.fragment,e)},m(e,n){de(t,e,n),a=!0},i(e){a||(I(t.$$.fragment,e),a=!0)},o(e){X(t.$$.fragment,e),a=!1},d(e){he(t,e)}}}function re(o){let t,a;return{c(){t=p("p"),a=T(o[3]),this.h()},l(e){t=_(e,"P",{class:!0});var n=d(t);a=w(n,o[3]),n.forEach(f),this.h()},h(){u(t,"class","error")},m(e,n){W(e,t,n),s(t,a)},p(e,n){n&8&&ge(a,e[3])},d(e){e&&f(t)}}}function ke(o){let t,a,e,n,P,k,L,q,O,h,M,K,B,y,R,Y,$,g,z,A,S,F,j,v,D,N,J,U,Q,Z,r=o[2]&&ne(),i=o[3]&&re(o);return{c(){r&&r.c(),t=b(),a=p("main"),e=p("form"),n=p("h1"),P=T("Login"),k=b(),L=p("span"),q=T("POCKET RELAY MANAGER"),O=b(),h=p("p"),M=T("Login to an existing account on the server"),K=b(),i&&i.c(),B=b(),y=p("label"),R=p("span"),Y=T("Email"),$=b(),g=p("input"),z=b(),A=p("label"),S=p("span"),F=T("Password"),j=b(),v=p("input"),D=b(),N=p("button"),J=T("Login"),this.h()},l(l){r&&r.l(l),t=E(l),a=_(l,"MAIN",{class:!0});var m=d(a);e=_(m,"FORM",{class:!0});var c=d(e);n=_(c,"H1",{});var x=d(n);P=w(x,"Login"),x.forEach(f),k=E(c),L=_(c,"SPAN",{class:!0});var ee=d(L);q=w(ee,"POCKET RELAY MANAGER"),ee.forEach(f),O=E(c),h=_(c,"P",{class:!0});var te=d(h);M=w(te,"Login to an existing account on the server"),te.forEach(f),K=E(c),i&&i.l(c),B=E(c),y=_(c,"LABEL",{class:!0});var C=d(y);R=_(C,"SPAN",{class:!0});var se=d(R);Y=w(se,"Email"),se.forEach(f),$=E(C),g=_(C,"INPUT",{class:!0,type:!0}),C.forEach(f),z=E(c),A=_(c,"LABEL",{class:!0});var G=d(A);S=_(G,"SPAN",{class:!0});var ae=d(S);F=w(ae,"Password"),ae.forEach(f),j=E(G),v=_(G,"INPUT",{class:!0,type:!0}),G.forEach(f),D=E(c),N=_(c,"BUTTON",{type:!0,class:!0});var le=d(N);J=w(le,"Login"),le.forEach(f),c.forEach(f),m.forEach(f),this.h()},h(){u(L,"class","ident"),u(h,"class","text"),u(R,"class","input__label"),u(g,"class","input__value"),u(g,"type","email"),g.required=!0,u(y,"class","input"),u(S,"class","input__label"),u(v,"class","input__value"),u(v,"type","password"),v.required=!0,u(A,"class","input"),u(N,"type","submit"),u(N,"class","button svelte-gpalk5"),u(e,"class","form card svelte-gpalk5"),u(a,"class","background svelte-gpalk5")},m(l,m){r&&r.m(l,m),W(l,t,m),W(l,a,m),s(a,e),s(e,n),s(n,P),s(e,k),s(e,L),s(L,q),s(e,O),s(e,h),s(h,M),s(e,K),i&&i.m(e,null),s(e,B),s(e,y),s(y,R),s(R,Y),s(y,$),s(y,g),H(g,o[0]),s(e,z),s(e,A),s(A,S),s(S,F),s(A,j),s(A,v),H(v,o[1]),s(e,D),s(e,N),s(N,J),U=!0,Q||(Z=[V(g,"input",o[5]),V(v,"input",o[6]),V(e,"submit",ce(o[4]))],Q=!0)},p(l,[m]){l[2]?r?m&4&&I(r,1):(r=ne(),r.c(),I(r,1),r.m(t.parentNode,t)):r&&(ve(),X(r,1,1,()=>{r=null}),fe()),l[3]?i?i.p(l,m):(i=re(l),i.c(),i.m(e,B)):i&&(i.d(1),i=null),m&1&&g.value!==l[0]&&H(g,l[0]),m&2&&v.value!==l[1]&&H(v,l[1])},i(l){U||(I(r),U=!0)},o(l){X(r),U=!1},d(l){r&&r.d(l),l&&f(t),l&&f(a),i&&i.d(),Q=!1,pe(Z)}}}function Ne(o,t,a){let e="",n="",P=!1,k=null;async function L(){a(3,k=null),a(2,P=!0);try{const{token:h}=await Pe(e,n);ye(h),await be("/")}catch(h){let M=h;console.error(M),a(3,k=M.text)}finally{a(2,P=!1)}}function q(){e=this.value,a(0,e)}function O(){n=this.value,a(1,n)}return[e,n,P,k,L,q,O]}class Se extends oe{constructor(t){super(),ie(this,t,Ne,ke,ue,{})}}export{Se as default};
